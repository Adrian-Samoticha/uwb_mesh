// TEMPLATES

use std::ops::{Add, Div, Mul, Neg, Sub};

pub fn generic_abs<T>(x: T) -> T
where
    T: PartialOrd + Neg<Output = T> + Copy + Default,
{
    if x > T::default() { x } else { -x }
}

pub fn generic_min<T>(x: T, y: T) -> T
where
    T: PartialOrd,
{
    if x < y { x } else { y }
}

pub fn generic_max<T>(x: T, y: T) -> T
where
    T: PartialOrd,
{
    if x > y { x } else { y }
}

pub fn generic_constrain<T>(x: T, min: T, max: T) -> T
where
    T: PartialOrd,
{
    generic_min(max, generic_max(min, x))
}

pub fn generic_is_constrained<T>(x: T, min: T, max: T) -> bool
where
    T: PartialOrd,
{
    x >= min && x <= max
}

pub fn generic_map<T>(x: T, from_low: T, from_high: T, to_low: T, to_high: T) -> T
where
    T: Copy + Sub<Output = T> + Mul<Output = T> + Div<Output = T> + Add<Output = T>,
{
    (x - from_low) * (to_high - to_low) / (from_high - from_low) + to_low
}

pub fn generic_constrained_map<T>(x: T, from_low: T, from_high: T, to_low: T, to_high: T) -> T
where
    T: Copy + PartialOrd + Sub<Output = T> + Mul<Output = T> + Div<Output = T> + Add<Output = T>,
{
    let result = (x - from_low) * (to_high - to_low) / (from_high - from_low) + to_low;
    if to_high > to_low {
        generic_constrain(result, to_low, to_high)
    } else {
        generic_constrain(result, to_high, to_low)
    }
}

pub fn interpolate<T>(a: T, b: T, pos: f64) -> T
where
    T: Into<f64> + From<f64> + Copy,
{
    let a_f = a.into();
    let b_f = b.into();
    T::from(a_f + (b_f - a_f) * pos)
}
