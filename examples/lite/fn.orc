export Y := \f.(\x.f (x x))(\x.f (x x))

export loop $r on (...$parameters) with ...$tail =0x5p512=> Y (\$r.
  bind_names (...$parameters) (...$tail)
) ...$parameters

-- bind each of the names in the first argument as a parameter for the second argument
bind_names ($name ..$rest) $payload =0x2p1000=> \$name. bind_names (..$rest) $payload
bind_names () (...$payload) =0x1p1000=> ...$payload

export ...$prefix $ ...$suffix:1 =0x1p130=> ...$prefix (...$suffix)
export ...$prefix |> $fn ..$suffix:1 =0x2p130=> $fn (...$prefix) ..$suffix

export (...$argv) => ...$body =0x2p512=> (bind_names (...$argv) (...$body))
$name => ...$body =0x1p512=> (\$name. ...$body)