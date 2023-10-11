import system::(io, directfs, async)
import std::(to_string, to_uint, inspect)

const folder_view := \path. \next. do{
  cps println $ "Contents of " ++ directfs::os_print path;
  cps entries = async::block_on $ directfs::read_dir path;
  cps list::enumerate entries
    |> list::map (pass \id. pass \name. \is_dir.
      println $ to_string id ++ ": " ++ directfs::os_print name ++ if is_dir then "/" else ""
    )
    |> list::chain;
  cps print "select an entry, or .. to move up: ";
  cps choice = readln;
  if (choice == "..") then do {
    let parent_path = directfs::pop_path path
      |> option::unwrap
      |> tuple::pick 0 2;
    next parent_path
  } else do {
    cps subname, is_dir = to_uint choice
      |> (list::get entries)
      |> option::unwrap;
    let subpath = directfs::join_paths path subname;
    if is_dir then next subpath
    else do {
      cps file = async::block_on $ directfs::read_file subpath;
      cps contents = async::block_on $ io::read_string file;
      cps println contents;
      next path
    }
  }
}

const main := loop_over (path = directfs::cwd) {
  cps path = folder_view path;
}

