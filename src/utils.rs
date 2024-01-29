use core::arch::asm;
use core::fmt;

pub trait HasEndianness: Eq + PartialEq + Copy + Clone + Default {
    fn from_be(self) -> Self;
    fn from_le(self) -> Self;
    fn to_be(self) -> Self;
    fn to_le(self) -> Self;
}

macro_rules! impl_has_endianness {
    ($t: ty) => {
        impl HasEndianness for $t {
            fn from_be(self) -> Self {
                <$t>::from_be(self)
            }
            fn from_le(self) -> Self {
                <$t>::from_le(self)
            }
            fn to_be(self) -> Self {
                <$t>::from_be(self)
            }
            fn to_le(self) -> Self {
                <$t>::from_le(self)
            }
        }
    };
}

impl_has_endianness!(u8);
impl_has_endianness!(i8);
impl_has_endianness!(u16);
impl_has_endianness!(i16);
impl_has_endianness!(u32);
impl_has_endianness!(i32);
impl_has_endianness!(u64);
impl_has_endianness!(i64);
impl_has_endianness!(u128);
impl_has_endianness!(i128);

#[repr(transparent)]
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct Endian<T, E>(T, core::marker::PhantomData<E>);

impl<T, E> Endian<T, E> {
    pub const fn from_raw(native: T) -> Self {
        Endian(native, core::marker::PhantomData)
    }
}

impl<T: HasEndianness, E: Endianness> Endian<T, E> {
    pub fn new(native: T) -> Self {
        Endian(E::from_native(native), core::marker::PhantomData)
    }

    pub fn native(self) -> T {
        E::to_native(self.0)
    }

    pub fn raw(self) -> T {
        self.0
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct Big;
#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct Little;

pub trait Endianness {
    fn from_native<T: HasEndianness>(native: T) -> T;
    fn to_native<T: HasEndianness>(native: T) -> T;
}
impl Endianness for Big {
    fn from_native<T: HasEndianness>(native: T) -> T {
        native.to_be()
    }

    fn to_native<T: HasEndianness>(native: T) -> T {
        native.from_be()
    }
}

impl Endianness for Little {
    fn from_native<T: HasEndianness>(native: T) -> T {
        native.to_le()
    }

    fn to_native<T: HasEndianness>(native: T) -> T {
        native.from_le()
    }
}

impl<T: HasEndianness, D: Endianness> From<T> for Endian<T, D> {
    fn from(f: T) -> Self {
        Self::new(f)
    }
}

impl<T: HasEndianness + fmt::Display, D: Endianness> fmt::Display for Endian<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", D::to_native(self.0))
    }
}

impl<T: HasEndianness + fmt::Debug, D: Endianness> fmt::Debug for Endian<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", D::to_native(self.0))
    }
}

impl<T: HasEndianness + fmt::LowerHex, D: Endianness> fmt::LowerHex for Endian<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", D::to_native(self.0))
    }
}

/// Memory barrier
pub fn mb() {
    unsafe {
        asm!("dsb 0");
    }
}

pub fn current_core() -> usize {
    let core: usize;
    unsafe {
        asm!("mrs	{0}, MPIDR_EL1
              and	{0}, {0}, #16777215
              ", out(reg) core);
    }
    core
}
