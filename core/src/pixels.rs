/// An amount of logical pixels.
///
/// Normally used to represent an amount of space, or the size of something.
///
/// This type is normally asked as an argument in a generic way
/// (e.g. `impl Into<Pixels>`) and, since `Pixels` implements `From` both for
/// `f32` and `u16`, you should be able to provide both integers and float
/// literals as needed.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Pixels(pub f32);

impl Pixels {
    /// Zero pixels.
    pub const ZERO: Self = Self(0.0);

    /// One logical pixel.
    pub const ONE: Self = Self(1.0);

    /// Creates a pixel value.
    pub const fn new(amount: f32) -> Self {
        Self(amount)
    }

    /// Returns the raw logical-pixel value.
    pub const fn get(self) -> f32 {
        self.0
    }

    /// Returns a finite, non-negative value safe for layout arithmetic.
    pub fn normalized(self) -> Self {
        if self.0.is_finite() {
            Self(self.0.max(0.0))
        } else {
            Self::ZERO
        }
    }
}

impl From<f32> for Pixels {
    fn from(amount: f32) -> Self {
        Self(amount)
    }
}

impl From<u32> for Pixels {
    fn from(amount: u32) -> Self {
        Self(amount as f32)
    }
}

impl From<Pixels> for f32 {
    fn from(pixels: Pixels) -> Self {
        pixels.0
    }
}

macro_rules! pixel_operation {
    ($trait:ident::$method:ident for $rhs:ty => $body:expr) => {
        impl std::ops::$trait<$rhs> for Pixels {
            type Output = Pixels;

            fn $method(self, rhs: $rhs) -> Self::Output {
                Pixels(($body)(self.0, rhs))
            }
        }
    };
}

pixel_operation!(Add::add for Pixels => |left: f32, right: Pixels| left + right.0);
pixel_operation!(Add::add for f32 => |left: f32, right: f32| left + right);
pixel_operation!(Sub::sub for Pixels => |left: f32, right: Pixels| left - right.0);
pixel_operation!(Sub::sub for f32 => |left: f32, right: f32| left - right);
pixel_operation!(Mul::mul for Pixels => |left: f32, right: Pixels| left * right.0);
pixel_operation!(Mul::mul for f32 => |left: f32, right: f32| left * right);
pixel_operation!(Div::div for Pixels => |left: f32, right: Pixels| left / right.0);
pixel_operation!(Div::div for f32 => |left: f32, right: f32| left / right);
pixel_operation!(Div::div for u32 => |left: f32, right: u32| left / right as f32);

impl std::iter::Sum for Pixels {
    fn sum<I: Iterator<Item = Self>>(values: I) -> Self {
        Self(values.map(|pixels| pixels.0).sum())
    }
}
