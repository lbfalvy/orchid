import std::number::add
import std::known::*

-- convert a comma-separated list into a linked list, with support for trailing commas
export ::comma_list
( macro comma_list ( ...$head, ...$tail:1 )
  =0x2p254=> ( await_comma_list ( ...$head ) comma_list ( ...$tail ) )
)
( macro comma_list (...$only)
  =0x1p254=> ( list_item (...$only) list_end )
)
( macro ( await_comma_list $head $tail )
  =0x2p254=> ( list_item $head $tail )
)
( macro comma_list ()
  =0x1p254=> list_end
)
( macro comma_list (...$data,)
  =0x3p254=> comma_list (...$data)
)

-- convert a comma-separated list into a linked list, with support for trailing commas
export ::semi_list
( macro semi_list ( ...$head; ...$tail:1 )
  =0x2p254=> ( await_semi_list ( ...$head ) semi_list ( ...$tail ) )
)
( macro semi_list (...$only)
  =0x1p254=> ( list_item (...$only) list_end )
)
( macro ( await_semi_list $head $tail )
  =0x2p254=> ( list_item $head $tail )
)
( macro semi_list ()
  =0x1p254=> list_end
)
( macro semi_list (...$data;)
  =0x3p254=> semi_list (...$data)
)

-- calculate the length of a linked list
export ::length
( macro length ( list_item $discard $tail )
  =0x1p254=> await_length ( length $tail )
)
( macro await_length ( $len )
  =0x1p254=> (add 1 $len)
)
macro length list_end =0x1p254=> (0)


export ::error
( macro ( ..$prefix error $details ..$suffix )
  =0x2p255=> error $details
)
( macro [ ..$prefix error $details ..$suffix ]
  =0x2p255=> error $details
)
( macro { ..$prefix error $details ..$suffix }
  =0x2p255=> error $details
)
( macro error $details
  =0x1p255=> 
)

export ::leftover_error
( macro leftover_error $details
  =0x1p255=> error ( "Token fails to parse" $details )
)
