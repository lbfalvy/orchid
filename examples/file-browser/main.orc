import system::(io, directfs, async)
import std::proc::*
import std::(to_string, to_uint)

const folder_view := \path.\next. do{
  cps println $ "Contents of " ++ path;
  cps entries = async::block_on $ directfs::readdir path;
  cps list::enumerate entries
    |> list::map (pass \id. pass \name.\is_dir. (
      println $ to_string id ++ ": " ++ name ++ if is_dir then "/" else ""
    ))
    |> list::chain;
  cps print "select an entry, or .. to move up: ";
  cps choice = readln;
  let output = if choice == "..\n"
    then directfs::pop_path path
      |> option::unwrap
      |> tuple::pick 0 2
    else (
      to_uint choice
        |> (list::get entries)
        |> option::unwrap
        |> (directfs::join_paths path)
    );
  next output
}

const main := loop_over (path = "/home/lbfalvy/Code/orchid/examples") {
  cps path = folder_view path;
}

