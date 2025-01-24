use std::future::Future;

use super::coding::Coding;
use crate::helpers::enc_vec;

pub trait Request: Coding + Sized + Send + 'static {
	type Response: Coding + Send + 'static;
}

pub async fn respond<R: Request>(_: &R, rep: R::Response) -> Vec<u8> { enc_vec(&rep).await }
pub async fn respond_with<R: Request, F: Future<Output = R::Response>>(
	r: &R,
	f: impl FnOnce(&R) -> F,
) -> Vec<u8> {
	respond(r, f(r).await).await
}

pub trait Channel: 'static {
	type Req: Coding + Sized + Send + 'static;
	type Notif: Coding + Sized + Send + 'static;
}

pub trait MsgSet: Send + Sync + 'static {
	type In: Channel;
	type Out: Channel;
}
