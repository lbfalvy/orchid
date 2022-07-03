pub fn result_iter_collect<T, E>(i: &mut dyn Iterator<Item = Result<T, E>>)
-> (Vec<Option<T>>, Vec<Option<E>>) {
    i.fold((Vec::new(), Vec::new()), |(mut succ, mut err), mut next| {
        match next {
            Ok(res) => succ.push(Some(res)),
            Err(e) => err.push(Some(e))
        }
        (succ, err)
    })
}

pub fn recoverable_iter_collect<T, E>(i: &mut dyn Iterator<Item=(Option<T>, Vec<E>)>)
-> (Vec<Option<T>>, Vec<E>) {
    i.fold((Vec::new(), Vec::new()), |(mut succ, mut err), (res, mut errv)| {
        succ.push(res);
        err.append(&mut errv);
        (succ, err)
    })
}