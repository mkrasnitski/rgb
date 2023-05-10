pub trait BitExtract {
    fn bit(self, bit: u8) -> bool;
}

macro_rules! impl_bit_extract {
    ($($t:ty),*) => {
        $(
            impl BitExtract for $t {
                fn bit(self, bit: u8) -> bool {
                    self & (1 << bit) != 0
                }
            }
        )*
    };
}

impl_bit_extract!(u8, u16);
