use num_traits::{Bounded, NumCast, ToPrimitive, Zero};

fn clamped<Source, Target>(value: Source) -> Target
where
    Source: ToPrimitive + PartialOrd + Zero + Copy,
    Target: NumCast + Bounded + Zero,
{
    match Target::from(value) {
        Some(converted) => converted,
        None if value < Source::zero() => Target::min_value(),
        None if value > Source::zero() => Target::max_value(),
        None => Target::zero(),
    }
}

pub(crate) fn to_f32<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> f32 {
    clamped(value)
}

pub(crate) fn to_f64<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> f64 {
    clamped(value)
}

pub(crate) fn to_i32<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> i32 {
    clamped(value)
}

pub(crate) fn to_i64<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> i64 {
    clamped(value)
}

pub(crate) fn to_isize<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> isize {
    clamped(value)
}

pub(crate) fn to_u8<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> u8 {
    clamped(value)
}

pub(crate) fn to_u16<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> u16 {
    clamped(value)
}

pub(crate) fn to_u32<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> u32 {
    clamped(value)
}

pub(crate) fn to_u64<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> u64 {
    clamped(value)
}

pub(crate) fn to_usize<Source: ToPrimitive + PartialOrd + Zero + Copy>(value: Source) -> usize {
    clamped(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_range_values_convert_exactly() {
        assert_eq!(to_i32(12.9_f32), 12);
        assert_eq!(to_i32(-12.9_f32), -12);
        assert_eq!(to_u8(200_u32), 200);
        assert_eq!(to_usize(7_u64), 7);
        assert_eq!(to_f32(3_i32).to_bits(), 3.0_f32.to_bits());
        assert_eq!(
            to_f64(u64::MAX).to_bits(),
            18_446_744_073_709_551_615.0_f64.to_bits()
        );
    }

    #[test]
    fn out_of_range_values_stop_at_the_nearest_bound() {
        assert_eq!(to_i32(3.0e10_f32), i32::MAX);
        assert_eq!(to_i32(-3.0e10_f32), i32::MIN);
        assert_eq!(to_u32(-1.0_f32), 0);
        assert_eq!(to_u8(300_u32), u8::MAX);
        assert_eq!(to_u32(-5_i32), 0);
        assert_eq!(to_i32(usize::MAX), i32::MAX);
        assert_eq!(to_u64(u128::MAX), u64::MAX);
        assert_eq!(to_i64(f64::INFINITY), i64::MAX);
        assert_eq!(to_i64(f64::NEG_INFINITY), i64::MIN);
    }

    #[test]
    fn not_a_number_becomes_zero() {
        assert_eq!(to_i32(f32::NAN), 0);
        assert_eq!(to_u64(f64::NAN), 0);
    }
}
