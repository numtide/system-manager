// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use core::fmt::Debug;

use typenum::*;

/// A trait that defines generalised operations on a `Bits::Store` type.
pub trait BitOps {
    fn get(bits: &Self, index: usize) -> bool;
    fn set(bits: &mut Self, index: usize, value: bool) -> bool;
    fn len(bits: &Self) -> usize;
    fn first_index(bits: &Self) -> Option<usize>;
    fn bit_and(bits: &mut Self, other_bits: &Self);
    fn bit_or(bits: &mut Self, other_bits: &Self);
    fn bit_xor(bits: &mut Self, other_bits: &Self);
    fn invert(bits: &mut Self);
    fn make_mask(shift: usize) -> Self;
    #[cfg(feature = "std")]
    fn to_hex(bits: &Self) -> String;
}

impl BitOps for bool {
    #[inline]
    fn get(bits: &Self, index: usize) -> bool {
        debug_assert!(index == 0);
        *bits
    }

    #[inline]
    fn set(bits: &mut Self, index: usize, value: bool) -> bool {
        debug_assert!(index == 0);
        core::mem::replace(bits, value)
    }

    #[inline]
    fn len(bits: &Self) -> usize {
        if *bits {
            1
        } else {
            0
        }
    }

    #[inline]
    fn first_index(bits: &Self) -> Option<usize> {
        if *bits {
            Some(0)
        } else {
            None
        }
    }

    #[inline]
    fn bit_and(bits: &mut Self, other_bits: &Self) {
        *bits &= *other_bits;
    }

    #[inline]
    fn bit_or(bits: &mut Self, other_bits: &Self) {
        *bits |= *other_bits;
    }

    #[inline]
    fn bit_xor(bits: &mut Self, other_bits: &Self) {
        *bits ^= *other_bits;
    }

    #[inline]
    fn invert(bits: &mut Self) {
        *bits = !*bits;
    }

    #[inline]
    fn make_mask(shift: usize) -> Self {
        shift > 0
    }

    #[cfg(feature = "std")]
    fn to_hex(bits: &Self) -> String {
        if *bits {
            "1".to_owned()
        } else {
            "0".to_owned()
        }
    }
}

macro_rules! bitops_for {
    ($target:ty) => {
        impl BitOps for $target {
            #[inline]
            fn get(bits: &Self, index: usize) -> bool {
                bits & (1 << index) != 0
            }

            #[inline]
            fn set(bits: &mut Self, index: usize, value: bool) -> bool {
                let mask = 1 << index;
                let prev = *bits & mask;
                if value {
                    *bits |= mask;
                } else {
                    *bits &= !mask;
                }
                prev != 0
            }

            #[inline]
            fn len(bits: &Self) -> usize {
                bits.count_ones() as usize
            }

            #[inline]
            fn first_index(bits: &Self) -> Option<usize> {
                if *bits == 0 {
                    None
                } else {
                    Some(bits.trailing_zeros() as usize)
                }
            }

            #[inline]
            fn bit_and(bits: &mut Self, other_bits: &Self) {
                *bits &= *other_bits;
            }

            #[inline]
            fn bit_or(bits: &mut Self, other_bits: &Self) {
                *bits |= *other_bits;
            }

            #[inline]
            fn bit_xor(bits: &mut Self, other_bits: &Self) {
                *bits ^= *other_bits;
            }

            #[inline]
            fn invert(bits: &mut Self) {
                *bits = !*bits;
            }

            #[inline]
            fn make_mask(shift: usize) -> Self {
                (1 << shift) - 1
            }

            #[cfg(feature = "std")]
            fn to_hex(bits: &Self) -> String {
                format!("{:x}", bits)
            }
        }
    };
}

macro_rules! bitops_for_big {
    ($words:expr) => {
        impl BitOps for [u128; $words] {
            #[inline]
            fn get(bits: &Self, index: usize) -> bool {
                let word_index = index / 128;
                let index = index & 127;
                bits[word_index] & (1 << index) != 0
            }

            #[inline]
            fn set(bits: &mut Self, index: usize, value: bool) -> bool {
                let word_index = index / 128;
                let index = index & 127;

                let mask = 1 << (index & 127);
                let bits = &mut bits[word_index];
                let prev = *bits & mask;
                if value {
                    *bits |= mask;
                } else {
                    *bits &= !mask;
                }
                prev != 0
            }

            fn make_mask(shift: usize) -> Self {
                let word_index = shift / 128;
                let index = shift & 127;
                let mut out = [0; $words];
                for (chunk_index, chunk) in out.iter_mut().enumerate() {
                    if chunk_index < word_index {
                        *chunk = !0u128;
                    } else if chunk_index == word_index {
                        *chunk = (1 << index) - 1;
                    } else {
                        return out;
                    }
                }
                out
            }

            #[inline]
            fn len(bits: &Self) -> usize {
                bits.iter().fold(0, |acc, next| acc + next.count_ones()) as usize
            }

            #[inline]
            fn first_index(bits: &Self) -> Option<usize> {
                for (index, part) in bits.iter().enumerate() {
                    if *part != 0u128 {
                        return Some(part.trailing_zeros() as usize + (128 * index));
                    }
                }
                None
            }

            #[inline]
            fn bit_and(bits: &mut Self, other_bits: &Self) {
                for (left, right) in bits.iter_mut().zip(other_bits.iter()) {
                    *left &= *right;
                }
            }

            #[inline]
            fn bit_or(bits: &mut Self, other_bits: &Self) {
                for (left, right) in bits.iter_mut().zip(other_bits.iter()) {
                    *left |= *right;
                }
            }

            #[inline]
            fn bit_xor(bits: &mut Self, other_bits: &Self) {
                for (left, right) in bits.iter_mut().zip(other_bits.iter()) {
                    *left ^= *right;
                }
            }

            #[inline]
            fn invert(bits: &mut Self) {
                for chunk in bits.iter_mut() {
                    *chunk = !*chunk;
                }
            }

            #[cfg(feature = "std")]
            fn to_hex(bits: &Self) -> String {
                let mut out = String::new();
                for chunk in bits {
                    out += &format!("{:x}", chunk);
                }
                out
            }
        }
    };
}

bitops_for!(u8);
bitops_for!(u16);
bitops_for!(u32);
bitops_for!(u64);
bitops_for!(u128);

bitops_for_big!(2);
bitops_for_big!(3);
bitops_for_big!(4);
bitops_for_big!(5);
bitops_for_big!(6);
bitops_for_big!(7);
bitops_for_big!(8);

/// A type level number signifying the number of bits in a bitmap.
///
/// This trait is implemented for type level numbers from `U1` to `U1024`.
///
/// # Examples
///
/// ```rust
/// # #[macro_use] extern crate bitmaps;
/// # use bitmaps::Bits;
/// # use typenum::U10;
/// assert_eq!(
///     std::mem::size_of::<<U10 as Bits>::Store>(),
///     std::mem::size_of::<u16>()
/// );
/// ```
pub trait Bits: Unsigned {
    /// A primitive integer type suitable for storing this many bits.
    type Store: BitOps + Default + Copy + PartialEq + Debug;
}

impl Bits for U1 {
    type Store = bool;
}

macro_rules! bits_for {
    ($num:ty, $result:ty) => {
        impl Bits for $num {
            type Store = $result;
        }
    };
}

macro_rules! bits_for_big {
    ($num:ty, $words:expr) => {
        impl Bits for $num {
            type Store = [u128; $words];
        }
    };
}

bits_for!(U2, u8);
bits_for!(U3, u8);
bits_for!(U4, u8);
bits_for!(U5, u8);
bits_for!(U6, u8);
bits_for!(U7, u8);
bits_for!(U8, u8);
bits_for!(U9, u16);
bits_for!(U10, u16);
bits_for!(U11, u16);
bits_for!(U12, u16);
bits_for!(U13, u16);
bits_for!(U14, u16);
bits_for!(U15, u16);
bits_for!(U16, u16);
bits_for!(U17, u32);
bits_for!(U18, u32);
bits_for!(U19, u32);
bits_for!(U20, u32);
bits_for!(U21, u32);
bits_for!(U22, u32);
bits_for!(U23, u32);
bits_for!(U24, u32);
bits_for!(U25, u32);
bits_for!(U26, u32);
bits_for!(U27, u32);
bits_for!(U28, u32);
bits_for!(U29, u32);
bits_for!(U30, u32);
bits_for!(U31, u32);
bits_for!(U32, u32);
bits_for!(U33, u64);
bits_for!(U34, u64);
bits_for!(U35, u64);
bits_for!(U36, u64);
bits_for!(U37, u64);
bits_for!(U38, u64);
bits_for!(U39, u64);
bits_for!(U40, u64);
bits_for!(U41, u64);
bits_for!(U42, u64);
bits_for!(U43, u64);
bits_for!(U44, u64);
bits_for!(U45, u64);
bits_for!(U46, u64);
bits_for!(U47, u64);
bits_for!(U48, u64);
bits_for!(U49, u64);
bits_for!(U50, u64);
bits_for!(U51, u64);
bits_for!(U52, u64);
bits_for!(U53, u64);
bits_for!(U54, u64);
bits_for!(U55, u64);
bits_for!(U56, u64);
bits_for!(U57, u64);
bits_for!(U58, u64);
bits_for!(U59, u64);
bits_for!(U60, u64);
bits_for!(U61, u64);
bits_for!(U62, u64);
bits_for!(U63, u64);
bits_for!(U64, u64);
bits_for!(U65, u128);
bits_for!(U66, u128);
bits_for!(U67, u128);
bits_for!(U68, u128);
bits_for!(U69, u128);
bits_for!(U70, u128);
bits_for!(U71, u128);
bits_for!(U72, u128);
bits_for!(U73, u128);
bits_for!(U74, u128);
bits_for!(U75, u128);
bits_for!(U76, u128);
bits_for!(U77, u128);
bits_for!(U78, u128);
bits_for!(U79, u128);
bits_for!(U80, u128);
bits_for!(U81, u128);
bits_for!(U82, u128);
bits_for!(U83, u128);
bits_for!(U84, u128);
bits_for!(U85, u128);
bits_for!(U86, u128);
bits_for!(U87, u128);
bits_for!(U88, u128);
bits_for!(U89, u128);
bits_for!(U90, u128);
bits_for!(U91, u128);
bits_for!(U92, u128);
bits_for!(U93, u128);
bits_for!(U94, u128);
bits_for!(U95, u128);
bits_for!(U96, u128);
bits_for!(U97, u128);
bits_for!(U98, u128);
bits_for!(U99, u128);
bits_for!(U100, u128);
bits_for!(U101, u128);
bits_for!(U102, u128);
bits_for!(U103, u128);
bits_for!(U104, u128);
bits_for!(U105, u128);
bits_for!(U106, u128);
bits_for!(U107, u128);
bits_for!(U108, u128);
bits_for!(U109, u128);
bits_for!(U110, u128);
bits_for!(U111, u128);
bits_for!(U112, u128);
bits_for!(U113, u128);
bits_for!(U114, u128);
bits_for!(U115, u128);
bits_for!(U116, u128);
bits_for!(U117, u128);
bits_for!(U118, u128);
bits_for!(U119, u128);
bits_for!(U120, u128);
bits_for!(U121, u128);
bits_for!(U122, u128);
bits_for!(U123, u128);
bits_for!(U124, u128);
bits_for!(U125, u128);
bits_for!(U126, u128);
bits_for!(U127, u128);
bits_for!(U128, u128);

bits_for_big!(U129, 2);
bits_for_big!(U130, 2);
bits_for_big!(U131, 2);
bits_for_big!(U132, 2);
bits_for_big!(U133, 2);
bits_for_big!(U134, 2);
bits_for_big!(U135, 2);
bits_for_big!(U136, 2);
bits_for_big!(U137, 2);
bits_for_big!(U138, 2);
bits_for_big!(U139, 2);
bits_for_big!(U140, 2);
bits_for_big!(U141, 2);
bits_for_big!(U142, 2);
bits_for_big!(U143, 2);
bits_for_big!(U144, 2);
bits_for_big!(U145, 2);
bits_for_big!(U146, 2);
bits_for_big!(U147, 2);
bits_for_big!(U148, 2);
bits_for_big!(U149, 2);
bits_for_big!(U150, 2);
bits_for_big!(U151, 2);
bits_for_big!(U152, 2);
bits_for_big!(U153, 2);
bits_for_big!(U154, 2);
bits_for_big!(U155, 2);
bits_for_big!(U156, 2);
bits_for_big!(U157, 2);
bits_for_big!(U158, 2);
bits_for_big!(U159, 2);
bits_for_big!(U160, 2);
bits_for_big!(U161, 2);
bits_for_big!(U162, 2);
bits_for_big!(U163, 2);
bits_for_big!(U164, 2);
bits_for_big!(U165, 2);
bits_for_big!(U166, 2);
bits_for_big!(U167, 2);
bits_for_big!(U168, 2);
bits_for_big!(U169, 2);
bits_for_big!(U170, 2);
bits_for_big!(U171, 2);
bits_for_big!(U172, 2);
bits_for_big!(U173, 2);
bits_for_big!(U174, 2);
bits_for_big!(U175, 2);
bits_for_big!(U176, 2);
bits_for_big!(U177, 2);
bits_for_big!(U178, 2);
bits_for_big!(U179, 2);
bits_for_big!(U180, 2);
bits_for_big!(U181, 2);
bits_for_big!(U182, 2);
bits_for_big!(U183, 2);
bits_for_big!(U184, 2);
bits_for_big!(U185, 2);
bits_for_big!(U186, 2);
bits_for_big!(U187, 2);
bits_for_big!(U188, 2);
bits_for_big!(U189, 2);
bits_for_big!(U190, 2);
bits_for_big!(U191, 2);
bits_for_big!(U192, 2);
bits_for_big!(U193, 2);
bits_for_big!(U194, 2);
bits_for_big!(U195, 2);
bits_for_big!(U196, 2);
bits_for_big!(U197, 2);
bits_for_big!(U198, 2);
bits_for_big!(U199, 2);
bits_for_big!(U200, 2);
bits_for_big!(U201, 2);
bits_for_big!(U202, 2);
bits_for_big!(U203, 2);
bits_for_big!(U204, 2);
bits_for_big!(U205, 2);
bits_for_big!(U206, 2);
bits_for_big!(U207, 2);
bits_for_big!(U208, 2);
bits_for_big!(U209, 2);
bits_for_big!(U210, 2);
bits_for_big!(U211, 2);
bits_for_big!(U212, 2);
bits_for_big!(U213, 2);
bits_for_big!(U214, 2);
bits_for_big!(U215, 2);
bits_for_big!(U216, 2);
bits_for_big!(U217, 2);
bits_for_big!(U218, 2);
bits_for_big!(U219, 2);
bits_for_big!(U220, 2);
bits_for_big!(U221, 2);
bits_for_big!(U222, 2);
bits_for_big!(U223, 2);
bits_for_big!(U224, 2);
bits_for_big!(U225, 2);
bits_for_big!(U226, 2);
bits_for_big!(U227, 2);
bits_for_big!(U228, 2);
bits_for_big!(U229, 2);
bits_for_big!(U230, 2);
bits_for_big!(U231, 2);
bits_for_big!(U232, 2);
bits_for_big!(U233, 2);
bits_for_big!(U234, 2);
bits_for_big!(U235, 2);
bits_for_big!(U236, 2);
bits_for_big!(U237, 2);
bits_for_big!(U238, 2);
bits_for_big!(U239, 2);
bits_for_big!(U240, 2);
bits_for_big!(U241, 2);
bits_for_big!(U242, 2);
bits_for_big!(U243, 2);
bits_for_big!(U244, 2);
bits_for_big!(U245, 2);
bits_for_big!(U246, 2);
bits_for_big!(U247, 2);
bits_for_big!(U248, 2);
bits_for_big!(U249, 2);
bits_for_big!(U250, 2);
bits_for_big!(U251, 2);
bits_for_big!(U252, 2);
bits_for_big!(U253, 2);
bits_for_big!(U254, 2);
bits_for_big!(U255, 2);
bits_for_big!(U256, 2);

bits_for_big!(U257, 3);
bits_for_big!(U258, 3);
bits_for_big!(U259, 3);
bits_for_big!(U260, 3);
bits_for_big!(U261, 3);
bits_for_big!(U262, 3);
bits_for_big!(U263, 3);
bits_for_big!(U264, 3);
bits_for_big!(U265, 3);
bits_for_big!(U266, 3);
bits_for_big!(U267, 3);
bits_for_big!(U268, 3);
bits_for_big!(U269, 3);
bits_for_big!(U270, 3);
bits_for_big!(U271, 3);
bits_for_big!(U272, 3);
bits_for_big!(U273, 3);
bits_for_big!(U274, 3);
bits_for_big!(U275, 3);
bits_for_big!(U276, 3);
bits_for_big!(U277, 3);
bits_for_big!(U278, 3);
bits_for_big!(U279, 3);
bits_for_big!(U280, 3);
bits_for_big!(U281, 3);
bits_for_big!(U282, 3);
bits_for_big!(U283, 3);
bits_for_big!(U284, 3);
bits_for_big!(U285, 3);
bits_for_big!(U286, 3);
bits_for_big!(U287, 3);
bits_for_big!(U288, 3);
bits_for_big!(U289, 3);
bits_for_big!(U290, 3);
bits_for_big!(U291, 3);
bits_for_big!(U292, 3);
bits_for_big!(U293, 3);
bits_for_big!(U294, 3);
bits_for_big!(U295, 3);
bits_for_big!(U296, 3);
bits_for_big!(U297, 3);
bits_for_big!(U298, 3);
bits_for_big!(U299, 3);
bits_for_big!(U300, 3);
bits_for_big!(U301, 3);
bits_for_big!(U302, 3);
bits_for_big!(U303, 3);
bits_for_big!(U304, 3);
bits_for_big!(U305, 3);
bits_for_big!(U306, 3);
bits_for_big!(U307, 3);
bits_for_big!(U308, 3);
bits_for_big!(U309, 3);
bits_for_big!(U310, 3);
bits_for_big!(U311, 3);
bits_for_big!(U312, 3);
bits_for_big!(U313, 3);
bits_for_big!(U314, 3);
bits_for_big!(U315, 3);
bits_for_big!(U316, 3);
bits_for_big!(U317, 3);
bits_for_big!(U318, 3);
bits_for_big!(U319, 3);
bits_for_big!(U320, 3);
bits_for_big!(U321, 3);
bits_for_big!(U322, 3);
bits_for_big!(U323, 3);
bits_for_big!(U324, 3);
bits_for_big!(U325, 3);
bits_for_big!(U326, 3);
bits_for_big!(U327, 3);
bits_for_big!(U328, 3);
bits_for_big!(U329, 3);
bits_for_big!(U330, 3);
bits_for_big!(U331, 3);
bits_for_big!(U332, 3);
bits_for_big!(U333, 3);
bits_for_big!(U334, 3);
bits_for_big!(U335, 3);
bits_for_big!(U336, 3);
bits_for_big!(U337, 3);
bits_for_big!(U338, 3);
bits_for_big!(U339, 3);
bits_for_big!(U340, 3);
bits_for_big!(U341, 3);
bits_for_big!(U342, 3);
bits_for_big!(U343, 3);
bits_for_big!(U344, 3);
bits_for_big!(U345, 3);
bits_for_big!(U346, 3);
bits_for_big!(U347, 3);
bits_for_big!(U348, 3);
bits_for_big!(U349, 3);
bits_for_big!(U350, 3);
bits_for_big!(U351, 3);
bits_for_big!(U352, 3);
bits_for_big!(U353, 3);
bits_for_big!(U354, 3);
bits_for_big!(U355, 3);
bits_for_big!(U356, 3);
bits_for_big!(U357, 3);
bits_for_big!(U358, 3);
bits_for_big!(U359, 3);
bits_for_big!(U360, 3);
bits_for_big!(U361, 3);
bits_for_big!(U362, 3);
bits_for_big!(U363, 3);
bits_for_big!(U364, 3);
bits_for_big!(U365, 3);
bits_for_big!(U366, 3);
bits_for_big!(U367, 3);
bits_for_big!(U368, 3);
bits_for_big!(U369, 3);
bits_for_big!(U370, 3);
bits_for_big!(U371, 3);
bits_for_big!(U372, 3);
bits_for_big!(U373, 3);
bits_for_big!(U374, 3);
bits_for_big!(U375, 3);
bits_for_big!(U376, 3);
bits_for_big!(U377, 3);
bits_for_big!(U378, 3);
bits_for_big!(U379, 3);
bits_for_big!(U380, 3);
bits_for_big!(U381, 3);
bits_for_big!(U382, 3);
bits_for_big!(U383, 3);
bits_for_big!(U384, 3);

bits_for_big!(U385, 4);
bits_for_big!(U386, 4);
bits_for_big!(U387, 4);
bits_for_big!(U388, 4);
bits_for_big!(U389, 4);
bits_for_big!(U390, 4);
bits_for_big!(U391, 4);
bits_for_big!(U392, 4);
bits_for_big!(U393, 4);
bits_for_big!(U394, 4);
bits_for_big!(U395, 4);
bits_for_big!(U396, 4);
bits_for_big!(U397, 4);
bits_for_big!(U398, 4);
bits_for_big!(U399, 4);
bits_for_big!(U400, 4);
bits_for_big!(U401, 4);
bits_for_big!(U402, 4);
bits_for_big!(U403, 4);
bits_for_big!(U404, 4);
bits_for_big!(U405, 4);
bits_for_big!(U406, 4);
bits_for_big!(U407, 4);
bits_for_big!(U408, 4);
bits_for_big!(U409, 4);
bits_for_big!(U410, 4);
bits_for_big!(U411, 4);
bits_for_big!(U412, 4);
bits_for_big!(U413, 4);
bits_for_big!(U414, 4);
bits_for_big!(U415, 4);
bits_for_big!(U416, 4);
bits_for_big!(U417, 4);
bits_for_big!(U418, 4);
bits_for_big!(U419, 4);
bits_for_big!(U420, 4);
bits_for_big!(U421, 4);
bits_for_big!(U422, 4);
bits_for_big!(U423, 4);
bits_for_big!(U424, 4);
bits_for_big!(U425, 4);
bits_for_big!(U426, 4);
bits_for_big!(U427, 4);
bits_for_big!(U428, 4);
bits_for_big!(U429, 4);
bits_for_big!(U430, 4);
bits_for_big!(U431, 4);
bits_for_big!(U432, 4);
bits_for_big!(U433, 4);
bits_for_big!(U434, 4);
bits_for_big!(U435, 4);
bits_for_big!(U436, 4);
bits_for_big!(U437, 4);
bits_for_big!(U438, 4);
bits_for_big!(U439, 4);
bits_for_big!(U440, 4);
bits_for_big!(U441, 4);
bits_for_big!(U442, 4);
bits_for_big!(U443, 4);
bits_for_big!(U444, 4);
bits_for_big!(U445, 4);
bits_for_big!(U446, 4);
bits_for_big!(U447, 4);
bits_for_big!(U448, 4);
bits_for_big!(U449, 4);
bits_for_big!(U450, 4);
bits_for_big!(U451, 4);
bits_for_big!(U452, 4);
bits_for_big!(U453, 4);
bits_for_big!(U454, 4);
bits_for_big!(U455, 4);
bits_for_big!(U456, 4);
bits_for_big!(U457, 4);
bits_for_big!(U458, 4);
bits_for_big!(U459, 4);
bits_for_big!(U460, 4);
bits_for_big!(U461, 4);
bits_for_big!(U462, 4);
bits_for_big!(U463, 4);
bits_for_big!(U464, 4);
bits_for_big!(U465, 4);
bits_for_big!(U466, 4);
bits_for_big!(U467, 4);
bits_for_big!(U468, 4);
bits_for_big!(U469, 4);
bits_for_big!(U470, 4);
bits_for_big!(U471, 4);
bits_for_big!(U472, 4);
bits_for_big!(U473, 4);
bits_for_big!(U474, 4);
bits_for_big!(U475, 4);
bits_for_big!(U476, 4);
bits_for_big!(U477, 4);
bits_for_big!(U478, 4);
bits_for_big!(U479, 4);
bits_for_big!(U480, 4);
bits_for_big!(U481, 4);
bits_for_big!(U482, 4);
bits_for_big!(U483, 4);
bits_for_big!(U484, 4);
bits_for_big!(U485, 4);
bits_for_big!(U486, 4);
bits_for_big!(U487, 4);
bits_for_big!(U488, 4);
bits_for_big!(U489, 4);
bits_for_big!(U490, 4);
bits_for_big!(U491, 4);
bits_for_big!(U492, 4);
bits_for_big!(U493, 4);
bits_for_big!(U494, 4);
bits_for_big!(U495, 4);
bits_for_big!(U496, 4);
bits_for_big!(U497, 4);
bits_for_big!(U498, 4);
bits_for_big!(U499, 4);
bits_for_big!(U500, 4);
bits_for_big!(U501, 4);
bits_for_big!(U502, 4);
bits_for_big!(U503, 4);
bits_for_big!(U504, 4);
bits_for_big!(U505, 4);
bits_for_big!(U506, 4);
bits_for_big!(U507, 4);
bits_for_big!(U508, 4);
bits_for_big!(U509, 4);
bits_for_big!(U510, 4);
bits_for_big!(U511, 4);
bits_for_big!(U512, 4);

bits_for_big!(U513, 5);
bits_for_big!(U514, 5);
bits_for_big!(U515, 5);
bits_for_big!(U516, 5);
bits_for_big!(U517, 5);
bits_for_big!(U518, 5);
bits_for_big!(U519, 5);
bits_for_big!(U520, 5);
bits_for_big!(U521, 5);
bits_for_big!(U522, 5);
bits_for_big!(U523, 5);
bits_for_big!(U524, 5);
bits_for_big!(U525, 5);
bits_for_big!(U526, 5);
bits_for_big!(U527, 5);
bits_for_big!(U528, 5);
bits_for_big!(U529, 5);
bits_for_big!(U530, 5);
bits_for_big!(U531, 5);
bits_for_big!(U532, 5);
bits_for_big!(U533, 5);
bits_for_big!(U534, 5);
bits_for_big!(U535, 5);
bits_for_big!(U536, 5);
bits_for_big!(U537, 5);
bits_for_big!(U538, 5);
bits_for_big!(U539, 5);
bits_for_big!(U540, 5);
bits_for_big!(U541, 5);
bits_for_big!(U542, 5);
bits_for_big!(U543, 5);
bits_for_big!(U544, 5);
bits_for_big!(U545, 5);
bits_for_big!(U546, 5);
bits_for_big!(U547, 5);
bits_for_big!(U548, 5);
bits_for_big!(U549, 5);
bits_for_big!(U550, 5);
bits_for_big!(U551, 5);
bits_for_big!(U552, 5);
bits_for_big!(U553, 5);
bits_for_big!(U554, 5);
bits_for_big!(U555, 5);
bits_for_big!(U556, 5);
bits_for_big!(U557, 5);
bits_for_big!(U558, 5);
bits_for_big!(U559, 5);
bits_for_big!(U560, 5);
bits_for_big!(U561, 5);
bits_for_big!(U562, 5);
bits_for_big!(U563, 5);
bits_for_big!(U564, 5);
bits_for_big!(U565, 5);
bits_for_big!(U566, 5);
bits_for_big!(U567, 5);
bits_for_big!(U568, 5);
bits_for_big!(U569, 5);
bits_for_big!(U570, 5);
bits_for_big!(U571, 5);
bits_for_big!(U572, 5);
bits_for_big!(U573, 5);
bits_for_big!(U574, 5);
bits_for_big!(U575, 5);
bits_for_big!(U576, 5);
bits_for_big!(U577, 5);
bits_for_big!(U578, 5);
bits_for_big!(U579, 5);
bits_for_big!(U580, 5);
bits_for_big!(U581, 5);
bits_for_big!(U582, 5);
bits_for_big!(U583, 5);
bits_for_big!(U584, 5);
bits_for_big!(U585, 5);
bits_for_big!(U586, 5);
bits_for_big!(U587, 5);
bits_for_big!(U588, 5);
bits_for_big!(U589, 5);
bits_for_big!(U590, 5);
bits_for_big!(U591, 5);
bits_for_big!(U592, 5);
bits_for_big!(U593, 5);
bits_for_big!(U594, 5);
bits_for_big!(U595, 5);
bits_for_big!(U596, 5);
bits_for_big!(U597, 5);
bits_for_big!(U598, 5);
bits_for_big!(U599, 5);
bits_for_big!(U600, 5);
bits_for_big!(U601, 5);
bits_for_big!(U602, 5);
bits_for_big!(U603, 5);
bits_for_big!(U604, 5);
bits_for_big!(U605, 5);
bits_for_big!(U606, 5);
bits_for_big!(U607, 5);
bits_for_big!(U608, 5);
bits_for_big!(U609, 5);
bits_for_big!(U610, 5);
bits_for_big!(U611, 5);
bits_for_big!(U612, 5);
bits_for_big!(U613, 5);
bits_for_big!(U614, 5);
bits_for_big!(U615, 5);
bits_for_big!(U616, 5);
bits_for_big!(U617, 5);
bits_for_big!(U618, 5);
bits_for_big!(U619, 5);
bits_for_big!(U620, 5);
bits_for_big!(U621, 5);
bits_for_big!(U622, 5);
bits_for_big!(U623, 5);
bits_for_big!(U624, 5);
bits_for_big!(U625, 5);
bits_for_big!(U626, 5);
bits_for_big!(U627, 5);
bits_for_big!(U628, 5);
bits_for_big!(U629, 5);
bits_for_big!(U630, 5);
bits_for_big!(U631, 5);
bits_for_big!(U632, 5);
bits_for_big!(U633, 5);
bits_for_big!(U634, 5);
bits_for_big!(U635, 5);
bits_for_big!(U636, 5);
bits_for_big!(U637, 5);
bits_for_big!(U638, 5);
bits_for_big!(U639, 5);
bits_for_big!(U640, 5);

bits_for_big!(U641, 6);
bits_for_big!(U642, 6);
bits_for_big!(U643, 6);
bits_for_big!(U644, 6);
bits_for_big!(U645, 6);
bits_for_big!(U646, 6);
bits_for_big!(U647, 6);
bits_for_big!(U648, 6);
bits_for_big!(U649, 6);
bits_for_big!(U650, 6);
bits_for_big!(U651, 6);
bits_for_big!(U652, 6);
bits_for_big!(U653, 6);
bits_for_big!(U654, 6);
bits_for_big!(U655, 6);
bits_for_big!(U656, 6);
bits_for_big!(U657, 6);
bits_for_big!(U658, 6);
bits_for_big!(U659, 6);
bits_for_big!(U660, 6);
bits_for_big!(U661, 6);
bits_for_big!(U662, 6);
bits_for_big!(U663, 6);
bits_for_big!(U664, 6);
bits_for_big!(U665, 6);
bits_for_big!(U666, 6);
bits_for_big!(U667, 6);
bits_for_big!(U668, 6);
bits_for_big!(U669, 6);
bits_for_big!(U670, 6);
bits_for_big!(U671, 6);
bits_for_big!(U672, 6);
bits_for_big!(U673, 6);
bits_for_big!(U674, 6);
bits_for_big!(U675, 6);
bits_for_big!(U676, 6);
bits_for_big!(U677, 6);
bits_for_big!(U678, 6);
bits_for_big!(U679, 6);
bits_for_big!(U680, 6);
bits_for_big!(U681, 6);
bits_for_big!(U682, 6);
bits_for_big!(U683, 6);
bits_for_big!(U684, 6);
bits_for_big!(U685, 6);
bits_for_big!(U686, 6);
bits_for_big!(U687, 6);
bits_for_big!(U688, 6);
bits_for_big!(U689, 6);
bits_for_big!(U690, 6);
bits_for_big!(U691, 6);
bits_for_big!(U692, 6);
bits_for_big!(U693, 6);
bits_for_big!(U694, 6);
bits_for_big!(U695, 6);
bits_for_big!(U696, 6);
bits_for_big!(U697, 6);
bits_for_big!(U698, 6);
bits_for_big!(U699, 6);
bits_for_big!(U700, 6);
bits_for_big!(U701, 6);
bits_for_big!(U702, 6);
bits_for_big!(U703, 6);
bits_for_big!(U704, 6);
bits_for_big!(U705, 6);
bits_for_big!(U706, 6);
bits_for_big!(U707, 6);
bits_for_big!(U708, 6);
bits_for_big!(U709, 6);
bits_for_big!(U710, 6);
bits_for_big!(U711, 6);
bits_for_big!(U712, 6);
bits_for_big!(U713, 6);
bits_for_big!(U714, 6);
bits_for_big!(U715, 6);
bits_for_big!(U716, 6);
bits_for_big!(U717, 6);
bits_for_big!(U718, 6);
bits_for_big!(U719, 6);
bits_for_big!(U720, 6);
bits_for_big!(U721, 6);
bits_for_big!(U722, 6);
bits_for_big!(U723, 6);
bits_for_big!(U724, 6);
bits_for_big!(U725, 6);
bits_for_big!(U726, 6);
bits_for_big!(U727, 6);
bits_for_big!(U728, 6);
bits_for_big!(U729, 6);
bits_for_big!(U730, 6);
bits_for_big!(U731, 6);
bits_for_big!(U732, 6);
bits_for_big!(U733, 6);
bits_for_big!(U734, 6);
bits_for_big!(U735, 6);
bits_for_big!(U736, 6);
bits_for_big!(U737, 6);
bits_for_big!(U738, 6);
bits_for_big!(U739, 6);
bits_for_big!(U740, 6);
bits_for_big!(U741, 6);
bits_for_big!(U742, 6);
bits_for_big!(U743, 6);
bits_for_big!(U744, 6);
bits_for_big!(U745, 6);
bits_for_big!(U746, 6);
bits_for_big!(U747, 6);
bits_for_big!(U748, 6);
bits_for_big!(U749, 6);
bits_for_big!(U750, 6);
bits_for_big!(U751, 6);
bits_for_big!(U752, 6);
bits_for_big!(U753, 6);
bits_for_big!(U754, 6);
bits_for_big!(U755, 6);
bits_for_big!(U756, 6);
bits_for_big!(U757, 6);
bits_for_big!(U758, 6);
bits_for_big!(U759, 6);
bits_for_big!(U760, 6);
bits_for_big!(U761, 6);
bits_for_big!(U762, 6);
bits_for_big!(U763, 6);
bits_for_big!(U764, 6);
bits_for_big!(U765, 6);
bits_for_big!(U766, 6);
bits_for_big!(U767, 6);
bits_for_big!(U768, 6);

bits_for_big!(U769, 7);
bits_for_big!(U770, 7);
bits_for_big!(U771, 7);
bits_for_big!(U772, 7);
bits_for_big!(U773, 7);
bits_for_big!(U774, 7);
bits_for_big!(U775, 7);
bits_for_big!(U776, 7);
bits_for_big!(U777, 7);
bits_for_big!(U778, 7);
bits_for_big!(U779, 7);
bits_for_big!(U780, 7);
bits_for_big!(U781, 7);
bits_for_big!(U782, 7);
bits_for_big!(U783, 7);
bits_for_big!(U784, 7);
bits_for_big!(U785, 7);
bits_for_big!(U786, 7);
bits_for_big!(U787, 7);
bits_for_big!(U788, 7);
bits_for_big!(U789, 7);
bits_for_big!(U790, 7);
bits_for_big!(U791, 7);
bits_for_big!(U792, 7);
bits_for_big!(U793, 7);
bits_for_big!(U794, 7);
bits_for_big!(U795, 7);
bits_for_big!(U796, 7);
bits_for_big!(U797, 7);
bits_for_big!(U798, 7);
bits_for_big!(U799, 7);
bits_for_big!(U800, 7);
bits_for_big!(U801, 7);
bits_for_big!(U802, 7);
bits_for_big!(U803, 7);
bits_for_big!(U804, 7);
bits_for_big!(U805, 7);
bits_for_big!(U806, 7);
bits_for_big!(U807, 7);
bits_for_big!(U808, 7);
bits_for_big!(U809, 7);
bits_for_big!(U810, 7);
bits_for_big!(U811, 7);
bits_for_big!(U812, 7);
bits_for_big!(U813, 7);
bits_for_big!(U814, 7);
bits_for_big!(U815, 7);
bits_for_big!(U816, 7);
bits_for_big!(U817, 7);
bits_for_big!(U818, 7);
bits_for_big!(U819, 7);
bits_for_big!(U820, 7);
bits_for_big!(U821, 7);
bits_for_big!(U822, 7);
bits_for_big!(U823, 7);
bits_for_big!(U824, 7);
bits_for_big!(U825, 7);
bits_for_big!(U826, 7);
bits_for_big!(U827, 7);
bits_for_big!(U828, 7);
bits_for_big!(U829, 7);
bits_for_big!(U830, 7);
bits_for_big!(U831, 7);
bits_for_big!(U832, 7);
bits_for_big!(U833, 7);
bits_for_big!(U834, 7);
bits_for_big!(U835, 7);
bits_for_big!(U836, 7);
bits_for_big!(U837, 7);
bits_for_big!(U838, 7);
bits_for_big!(U839, 7);
bits_for_big!(U840, 7);
bits_for_big!(U841, 7);
bits_for_big!(U842, 7);
bits_for_big!(U843, 7);
bits_for_big!(U844, 7);
bits_for_big!(U845, 7);
bits_for_big!(U846, 7);
bits_for_big!(U847, 7);
bits_for_big!(U848, 7);
bits_for_big!(U849, 7);
bits_for_big!(U850, 7);
bits_for_big!(U851, 7);
bits_for_big!(U852, 7);
bits_for_big!(U853, 7);
bits_for_big!(U854, 7);
bits_for_big!(U855, 7);
bits_for_big!(U856, 7);
bits_for_big!(U857, 7);
bits_for_big!(U858, 7);
bits_for_big!(U859, 7);
bits_for_big!(U860, 7);
bits_for_big!(U861, 7);
bits_for_big!(U862, 7);
bits_for_big!(U863, 7);
bits_for_big!(U864, 7);
bits_for_big!(U865, 7);
bits_for_big!(U866, 7);
bits_for_big!(U867, 7);
bits_for_big!(U868, 7);
bits_for_big!(U869, 7);
bits_for_big!(U870, 7);
bits_for_big!(U871, 7);
bits_for_big!(U872, 7);
bits_for_big!(U873, 7);
bits_for_big!(U874, 7);
bits_for_big!(U875, 7);
bits_for_big!(U876, 7);
bits_for_big!(U877, 7);
bits_for_big!(U878, 7);
bits_for_big!(U879, 7);
bits_for_big!(U880, 7);
bits_for_big!(U881, 7);
bits_for_big!(U882, 7);
bits_for_big!(U883, 7);
bits_for_big!(U884, 7);
bits_for_big!(U885, 7);
bits_for_big!(U886, 7);
bits_for_big!(U887, 7);
bits_for_big!(U888, 7);
bits_for_big!(U889, 7);
bits_for_big!(U890, 7);
bits_for_big!(U891, 7);
bits_for_big!(U892, 7);
bits_for_big!(U893, 7);
bits_for_big!(U894, 7);
bits_for_big!(U895, 7);
bits_for_big!(U896, 7);

bits_for_big!(U897, 8);
bits_for_big!(U898, 8);
bits_for_big!(U899, 8);
bits_for_big!(U900, 8);
bits_for_big!(U901, 8);
bits_for_big!(U902, 8);
bits_for_big!(U903, 8);
bits_for_big!(U904, 8);
bits_for_big!(U905, 8);
bits_for_big!(U906, 8);
bits_for_big!(U907, 8);
bits_for_big!(U908, 8);
bits_for_big!(U909, 8);
bits_for_big!(U910, 8);
bits_for_big!(U911, 8);
bits_for_big!(U912, 8);
bits_for_big!(U913, 8);
bits_for_big!(U914, 8);
bits_for_big!(U915, 8);
bits_for_big!(U916, 8);
bits_for_big!(U917, 8);
bits_for_big!(U918, 8);
bits_for_big!(U919, 8);
bits_for_big!(U920, 8);
bits_for_big!(U921, 8);
bits_for_big!(U922, 8);
bits_for_big!(U923, 8);
bits_for_big!(U924, 8);
bits_for_big!(U925, 8);
bits_for_big!(U926, 8);
bits_for_big!(U927, 8);
bits_for_big!(U928, 8);
bits_for_big!(U929, 8);
bits_for_big!(U930, 8);
bits_for_big!(U931, 8);
bits_for_big!(U932, 8);
bits_for_big!(U933, 8);
bits_for_big!(U934, 8);
bits_for_big!(U935, 8);
bits_for_big!(U936, 8);
bits_for_big!(U937, 8);
bits_for_big!(U938, 8);
bits_for_big!(U939, 8);
bits_for_big!(U940, 8);
bits_for_big!(U941, 8);
bits_for_big!(U942, 8);
bits_for_big!(U943, 8);
bits_for_big!(U944, 8);
bits_for_big!(U945, 8);
bits_for_big!(U946, 8);
bits_for_big!(U947, 8);
bits_for_big!(U948, 8);
bits_for_big!(U949, 8);
bits_for_big!(U950, 8);
bits_for_big!(U951, 8);
bits_for_big!(U952, 8);
bits_for_big!(U953, 8);
bits_for_big!(U954, 8);
bits_for_big!(U955, 8);
bits_for_big!(U956, 8);
bits_for_big!(U957, 8);
bits_for_big!(U958, 8);
bits_for_big!(U959, 8);
bits_for_big!(U960, 8);
bits_for_big!(U961, 8);
bits_for_big!(U962, 8);
bits_for_big!(U963, 8);
bits_for_big!(U964, 8);
bits_for_big!(U965, 8);
bits_for_big!(U966, 8);
bits_for_big!(U967, 8);
bits_for_big!(U968, 8);
bits_for_big!(U969, 8);
bits_for_big!(U970, 8);
bits_for_big!(U971, 8);
bits_for_big!(U972, 8);
bits_for_big!(U973, 8);
bits_for_big!(U974, 8);
bits_for_big!(U975, 8);
bits_for_big!(U976, 8);
bits_for_big!(U977, 8);
bits_for_big!(U978, 8);
bits_for_big!(U979, 8);
bits_for_big!(U980, 8);
bits_for_big!(U981, 8);
bits_for_big!(U982, 8);
bits_for_big!(U983, 8);
bits_for_big!(U984, 8);
bits_for_big!(U985, 8);
bits_for_big!(U986, 8);
bits_for_big!(U987, 8);
bits_for_big!(U988, 8);
bits_for_big!(U989, 8);
bits_for_big!(U990, 8);
bits_for_big!(U991, 8);
bits_for_big!(U992, 8);
bits_for_big!(U993, 8);
bits_for_big!(U994, 8);
bits_for_big!(U995, 8);
bits_for_big!(U996, 8);
bits_for_big!(U997, 8);
bits_for_big!(U998, 8);
bits_for_big!(U999, 8);
bits_for_big!(U1000, 8);
bits_for_big!(U1001, 8);
bits_for_big!(U1002, 8);
bits_for_big!(U1003, 8);
bits_for_big!(U1004, 8);
bits_for_big!(U1005, 8);
bits_for_big!(U1006, 8);
bits_for_big!(U1007, 8);
bits_for_big!(U1008, 8);
bits_for_big!(U1009, 8);
bits_for_big!(U1010, 8);
bits_for_big!(U1011, 8);
bits_for_big!(U1012, 8);
bits_for_big!(U1013, 8);
bits_for_big!(U1014, 8);
bits_for_big!(U1015, 8);
bits_for_big!(U1016, 8);
bits_for_big!(U1017, 8);
bits_for_big!(U1018, 8);
bits_for_big!(U1019, 8);
bits_for_big!(U1020, 8);
bits_for_big!(U1021, 8);
bits_for_big!(U1022, 8);
bits_for_big!(U1023, 8);
bits_for_big!(U1024, 8);
