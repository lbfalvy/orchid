use itertools::Itertools;
use orchidlang::pipeline::project::{ItemKind, ProjItem, ProjectMod};
use orchidlang::tree::{ModEntry, ModMember};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ProjPrintOpts {
  pub width: u16,
  pub hide_locations: bool,
}

fn indent(amount: u16) -> String { "  ".repeat(amount.into()) }

pub fn print_proj_mod(module: &ProjectMod, lvl: u16, opts: ProjPrintOpts) -> String {
  let mut acc = String::new();
  let tab = indent(lvl);
  for (key, ModEntry { member, x }) in &module.entries {
    let mut line_acc = String::new();
    for c in &x.comments {
      line_acc += &format!("{tab}, --[|{}|]--\n", c);
    }
    if x.exported {
      line_acc += &format!("{tab}export ");
    } else {
      line_acc += &tab
    }
    match member {
      ModMember::Sub(module) => {
        line_acc += &format!("module {key} {{\n");
        line_acc += &print_proj_mod(module, lvl + 1, opts);
        line_acc += &format!("{tab}}}");
      },
      ModMember::Item(ProjItem { kind: ItemKind::None }) => {
        line_acc += &format!("keyword {key}");
      },
      ModMember::Item(ProjItem { kind: ItemKind::Alias(tgt) }) => {
        line_acc += &format!("alias {key} => {tgt}");
      },
      ModMember::Item(ProjItem { kind: ItemKind::Const(val) }) => {
        line_acc += &format!("const {key} := {val}");
      },
    }
    if !x.locations.is_empty() && !opts.hide_locations {
      let locs = x.locations.iter().map(|l| l.to_string()).join(", ");
      let line_len = line_acc.split('\n').last().unwrap().len();
      match usize::from(opts.width).checked_sub(locs.len() + line_len + 4) {
        Some(padding) => line_acc += &" ".repeat(padding),
        None => line_acc += &format!("\n{tab}  @ "),
      }
      line_acc += &locs;
    }
    line_acc += "\n";
    acc += &line_acc
  }
  acc
}
