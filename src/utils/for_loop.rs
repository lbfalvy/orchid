/// Imitates a regular for loop with an exit clause using Rust's `loop` keyword.
/// This macro brings the break value to all existing Rust loops, by allowing you to specify
/// an exit expression in case the loop was broken by the condition and not an explicit `break`.
/// 
/// Since the exit expression can also be a block, this also allows you to execute other code when
/// the condition fails. This can also be used to re-enter the loop with an explicit `continue`
/// statement.
/// 
/// The macro also adds support for classic for loops familiar to everyone since C, except with
/// the addition of an exit statement these too can be turned into expressions.
/// 
/// ```
/// xloop!(for i in 0..10; {
///     connection.try_connect()
///     if connection.ready() {
///         break Some(connection)
///     }
/// }; None)
/// ```
/// 
/// While loop with reentry. This is a very convoluted example but displays the idea quite clearly.
/// 
/// ```
/// xloop!(while socket.is_open(); {
///     let (data, is_end) = socket.read();
///     all_data.append(data)
///     if is_end { break Ok(all_data) }
/// }; {
///     if let Ok(new_sock) = open_socket(socket.position()) {
///         new_sock.set_position(socket.position());
///         socket = new_sock;
///         continue
///     } else {
///         Err(DownloadError::ConnectionLost)
///     }
/// })
/// ```
/// 
/// CUDA algorythm for O(log n) summation using a C loop
/// 
/// ```
/// xloop!(let mut leap = 1; own_id*2 + leap < batch_size; leap *= 2; {
///     batch[own_id*2] += batch[own_id*2 + leap]
/// })
/// ```
/// 
/// The above loop isn't used as an expression, but an exit expression - or block - can be added
/// to these as well just like the others. In all cases the exit expression is optional, its
/// default value is `()`.
/// 
/// **todo** find a valid use case for While let for a demo
#[macro_export]
macro_rules! xloop {
    (for $p:pat in $it:expr; $body:stmt) => {
        xloop!(for $p in $it; $body; ())
    };
    (for $p:pat in $it:expr; $body:stmt; $exit:stmt) => {
        {
            let mut __xloop__ = $it.into_iter();
            xloop!(let Some($p) = __xloop__.next(); $body; $exit)
        }
    };
    (let $p:pat = $e:expr; $body:stmt) => {
        xloop!(let $p = $e; $body; ())
    };
    (let $p:pat = $e:expr; $body:stmt; $exit:stmt) => {
        {
            loop {
                if let $p = $e { $body }
                else { break { $exit } }
            }
        }
    };
    (while $cond:expr; $body:stmt) => {
        xloop!($cond; $body; ())
    };
    (while $cond:expr; $body:stmt; $exit:stmt) => {
        {
            loop {
                if $cond { break { $exit } }
                else { $body }
            }
        }
    };
    ($init:stmt; $cond:expr; $step:stmt; $body:stmt) => {
        xloop!(for ( $init; $cond; $step ) $body; ())
    };
    ($init:stmt; $cond:expr; $step:stmt; $body:stmt; $exit:stmt) => {
        { $init; xloop!(while !($cond); { $body; $step }; $exit) }
    };
}