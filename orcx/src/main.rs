use std::fs::File;
use std::io::{Read, Write};
use std::mem;
use std::process::{Command, ExitCode};
use std::rc::Rc;

use async_std::io::stdin;
use async_stream::try_stream;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use futures::{Stream, TryStreamExt, io};
use orchid_base::error::ReporterImpl;
use orchid_base::format::{FmtCtxImpl, Format, take_first};
use orchid_base::location::Pos;
use orchid_base::logging::{LogStrategy, Logger};
use orchid_base::macros::mtreev_fmt;
use orchid_base::parse::Snippet;
use orchid_base::tree::ttv_fmt;
use orchid_host::ctx::Ctx;
use orchid_host::execute::{ExecCtx, ExecResult};
use orchid_host::expr::mtreev_to_expr;
use orchid_host::extension::Extension;
use orchid_host::lex::lex;
use orchid_host::parse::{ParseCtxImpl, parse_items, parse_mtree};
use orchid_host::subprocess::ext_command;
use orchid_host::system::init_systems;
use substack::Substack;
use tokio::task::{LocalSet, spawn_local};

#[derive(Parser, Debug)]
#[command(version, about, long_about)]
pub struct Args {
	#[arg(short, long, env = "ORCHID_EXTENSIONS", value_delimiter = ';')]
	extension: Vec<Utf8PathBuf>,
	#[arg(short, long, env = "ORCHID_DEFAULT_SYSTEMS", value_delimiter = ';')]
	system: Vec<String>,
	#[arg(short, long)]
	verbose: bool,
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
	Lex {
		#[arg(short, long)]
		file: Utf8PathBuf,
	},
	Parse {
		#[arg(short, long)]
		file: Utf8PathBuf,
	},
	Repl,
	Execute {
		#[arg()]
		code: String,
	},
}

fn get_all_extensions<'a>(
	args: &'a Args,
	logger: &'a Logger,
	msg_logger: &'a Logger,
	ctx: &'a Ctx,
) -> impl Stream<Item = io::Result<Extension>> + 'a {
	try_stream! {
		for ext_path in args.extension.iter() {
			let exe = if cfg!(windows) {
				ext_path.with_extension("exe")
			} else {
				ext_path.clone()
			};
			let init = ext_command(Command::new(exe.as_os_str()), logger.clone(), msg_logger.clone(), ctx.clone()).await
				.unwrap();
			let ext = Extension::new(init, logger.clone(), msg_logger.clone(), ctx.clone())?;
			yield ext
		}
	}
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<ExitCode> {
	let mut code = ExitCode::SUCCESS;
	LocalSet::new()
		.run_until(async {
			let args = Args::parse();
			let ctx = &Ctx::new(Rc::new(|fut| mem::drop(spawn_local(fut))));
			let i = &ctx.i;
			let logger =
				Logger::new(if args.verbose { LogStrategy::StdErr } else { LogStrategy::Discard });
			let extensions = get_all_extensions(&args, &logger, &Logger::new(LogStrategy::Discard), ctx)
				.try_collect::<Vec<Extension>>()
				.await
				.unwrap();
			match args.command {
				Commands::Lex { file } => {
					let systems = init_systems(&args.system, &extensions).await.unwrap();
					let mut file = File::open(file.as_std_path()).unwrap();
					let mut buf = String::new();
					file.read_to_string(&mut buf).unwrap();
					let lexemes = lex(i.i(&buf).await, &systems, ctx).await.unwrap();
					println!("{}", take_first(&ttv_fmt(&lexemes, &FmtCtxImpl { i }).await, true))
				},
				Commands::Parse { file } => {
					let systems = init_systems(&args.system, &extensions).await.unwrap();
					let mut file = File::open(file.as_std_path()).unwrap();
					let mut buf = String::new();
					file.read_to_string(&mut buf).unwrap();
					let lexemes = lex(i.i(&buf).await, &systems, ctx).await.unwrap();
					let Some(first) = lexemes.first() else {
						println!("File empty!");
						return;
					};
					let reporter = ReporterImpl::new();
					let pctx = ParseCtxImpl { reporter: &reporter, systems: &systems };
					let snip = Snippet::new(first, &lexemes, i);
					let ptree = parse_items(&pctx, Substack::Bottom, snip).await.unwrap();
					if let Some(errv) = reporter.errv() {
						eprintln!("{errv}");
						code = ExitCode::FAILURE;
						return;
					}
					if ptree.is_empty() {
						eprintln!("File empty only after parsing, but no errors were reported");
						code = ExitCode::FAILURE;
						return;
					}
					for item in ptree {
						println!("{}", take_first(&item.print(&FmtCtxImpl { i }).await, true))
					}
				},
				Commands::Repl => loop {
					let systems = init_systems(&args.system, &extensions).await.unwrap();
					print!("\\.> ");
					std::io::stdout().flush().unwrap();
					let mut prompt = String::new();
					stdin().read_line(&mut prompt).await.unwrap();
					let lexemes = lex(i.i(prompt.trim()).await, &systems, ctx).await.unwrap();
					if args.verbose {
						println!("lexed: {}", take_first(&ttv_fmt(&lexemes, &FmtCtxImpl { i }).await, true));
					}
					let mtreev = parse_mtree(
						Snippet::new(&lexemes[0], &lexemes, i),
						Substack::Bottom.push(i.i("orcx").await).push(i.i("input").await),
					)
					.await
					.unwrap();
					if args.verbose {
						let fmt = mtreev_fmt(&mtreev, &FmtCtxImpl { i }).await;
						println!("parsed: {}", take_first(&fmt, true));
					}
					let expr = mtreev_to_expr(&mtreev, Substack::Bottom, ctx).await;
					let mut xctx = ExecCtx::new(ctx.clone(), logger.clone(), expr.at(Pos::None)).await;
					xctx.set_gas(Some(1000));
					xctx.execute().await;
					match xctx.result() {
						ExecResult::Value(val) =>
							println!("{}", take_first(&val.print(&FmtCtxImpl { i }).await, false)),
						ExecResult::Err(e) => println!("error: {e}"),
						ExecResult::Gas(_) => println!("Ran out of gas!"),
					}
				},
				Commands::Execute { code } => {
					let systems = init_systems(&args.system, &extensions).await.unwrap();
					let lexemes = lex(i.i(code.trim()).await, &systems, ctx).await.unwrap();
					if args.verbose {
						println!("lexed: {}", take_first(&ttv_fmt(&lexemes, &FmtCtxImpl { i }).await, true));
					}
					let mtreev =
						parse_mtree(Snippet::new(&lexemes[0], &lexemes, i), Substack::Bottom).await.unwrap();
					if args.verbose {
						let fmt = mtreev_fmt(&mtreev, &FmtCtxImpl { i }).await;
						println!("parsed: {}", take_first(&fmt, true));
					}
					let expr = mtreev_to_expr(&mtreev, Substack::Bottom, ctx).await;
					let mut xctx = ExecCtx::new(ctx.clone(), logger.clone(), expr.at(Pos::None)).await;
					xctx.set_gas(Some(1000));
					xctx.execute().await;
					match xctx.result() {
						ExecResult::Value(val) =>
							println!("{}", take_first(&val.print(&FmtCtxImpl { i }).await, false)),
						ExecResult::Err(e) => println!("error: {e}"),
						ExecResult::Gas(_) => println!("Ran out of gas!"),
					}
				},
			}
		})
		.await;
	Ok(code)
}
