use orchid_base::number::Numeric;
use orchid_extension::tree::{GenItem, fun, prefix};
use ordered_float::NotNan;

use super::num_atom::{Float, HomoArray, Int, Num};

pub fn gen_num_lib() -> Vec<GenItem> {
	prefix("std::number", [
		fun(true, "add", |a: Num, b: Num| async move {
			Num(match HomoArray::new([a.0, b.0]) {
				HomoArray::Int([a, b]) => Numeric::Int(a + b),
				HomoArray::Float([a, b]) => Numeric::Float(a + b),
			})
		}),
		fun(true, "neg", |a: Num| async move {
			Num(match a.0 {
				Numeric::Int(i) => Numeric::Int(-i),
				Numeric::Float(f) => Numeric::Float(-f),
			})
		}),
		fun(true, "mul", |a: Num, b: Num| async move {
			Num(match HomoArray::new([a.0, b.0]) {
				HomoArray::Int([a, b]) => Numeric::Int(a * b),
				HomoArray::Float([a, b]) => Numeric::Float(a * b),
			})
		}),
		fun(true, "idiv", |a: Int, b: Int| async move { Int(a.0 / b.0) }),
		fun(true, "imod", |a: Int, b: Int| async move { Int(a.0 % b.0) }),
		fun(true, "fdiv", |a: Float, b: Float| async move { Float(a.0 / b.0) }),
		fun(true, "fmod", |a: Float, b: Float| async move {
			Float(a.0 - NotNan::new((a.0 / b.0).trunc()).unwrap() * b.0)
		}),
	])
}
