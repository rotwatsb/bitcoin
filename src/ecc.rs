use libc::{c_void, c_int, c_char, c_ulong, c_long, c_uint, c_uchar, size_t};
use openssl-sys::{BIGNUM};

#[repr(C)]
pub struct EC_GROUP {
    pub meth: *const c_void,
    pub generator: *mut EC_POINT,
    pub order: *mut BIGNUM,
    pub cofactor: *mut BIGNUM,
    pub curve_name: c_int,
    pub asn1_flag: c_int,
    pub asn1_form: POINT_CONVERSION_FORM_T,
    pub seed: *mut c_uchar,
    pub seed_len: size_t,
    pub field: *mut BIGNUM,
    pub poly: [c_int; 6],
    pub a: *mut BIGNUM,
    pub b: *mut BIGNUM,
    pub a_is_minus3: c_int,
    pub field_data1: *mut c_void,
    pub field_data2: *mut c_void,
    
}

#[repr(C)]
pub struct EC_POINT {
    pub meth: *const c_void,
    pub x: *mut BIGNUM,
    pub y: *mut BIGNUM,
    pub z: *mut BIGNUM,
    pub z_is_one: c_int,
    pub custom_data: *mut c_void,
}

#[repr(C)]
pub enum POINT_CONVERSION_FORM_T {
    POINT_CONVERSION_COMPRESSED: c_int = 2,
    POINT_CONVERSION_UNCOMPRESSED: c_int = 4,
    POINT_CONVERSION_HYBRID: c_int = 6,
}
