export Y := \f.(\x.f (x x))(\x.f (x x))

export loop $r on (..$parameters) with ...$tail =0x5p129=> Y (\$r.
  bind_names (..$parameters) (...$tail)
) ..$parameters

-- bind each of the names in the first argument as a parameter for the second argument
bind_names ($name ..$rest) $payload =0x1p250=> \$name. bind_names (..$rest) $payload
bind_names () (...$payload) =0x1p250=> ...$payload

export ...$prefix $ ...$suffix:1 =0x1p38=> ...$prefix (...$suffix)
export ...$prefix |> $fn ..$suffix:1 =0x2p32=> $fn (...$prefix) ..$suffix

export (...$argv) => ...$body =0x2p129=> (bind_names (...$argv) (...$body))
$name => ...$body =0x1p129=> (\$name. ...$body)