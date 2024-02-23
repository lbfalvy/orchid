import system::(io, fs, async)
import std::(conv::(to_string, to_uint), inspect)

const folder_view := \path. do cps {
  cps println $ "Contents of ${path}";
  cps entries = async::block_on $ fs::read_dir path;
  cps list::enumerate entries
    |> list::map ((t[id, t[name, is_dir]]) =>
      println $ "${id}: ${name}" ++ if is_dir then "/" else ""
    )
    |> list::chain;
  cps choice = prompt "select an entry, or .. to move up: ";
  cps new_path = if choice == ".." then do cps {
    let t[parent_path, _] = fs::pop_path path
      |> option::assume;
    cps pass parent_path;
  } else do cps {
    let t[subname, is_dir] = to_uint choice
      |> (list::get entries)
      |> option::assume;
    let subpath = fs::join_paths path subname;
    cps if is_dir then pass subpath else do cps {
      cps file = async::block_on $ fs::read_file subpath;
      cps contents = async::block_on $ io::read_string file;
      cps println contents;
      cps _ = prompt "Hit Enter to return to the parent directory: ";
      cps pass path;
    };
  };
  cps pass new_path;
}

const main := loop_over (path = fs::cwd) {
  cps path = folder_view path;
}
