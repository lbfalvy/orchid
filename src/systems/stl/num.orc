export operators[ + - * % / ]

macro ...$a + ...$b =0x2p36=> (add (...$a) (...$b))
macro ...$a - ...$b:1 =0x2p36=> (subtract (...$a) (...$b))
macro ...$a * ...$b =0x1p36=> (multiply (...$a) (...$b))
macro ...$a % ...$b:1 =0x1p36=> (remainder (...$a) (...$b))
macro ...$a / ...$b:1 =0x1p36=> (divide (...$a) (...$b))
