#![allow(
    // We choose to have narrow "unsafe" blocks instead of marking entire
    // functions as unsafe. Even the example in clippy's docs at
    // https://rust-lang.github.io/rust-clippy/master/index.html#not_unsafe_ptr_arg_deref
    // cause a rustc warning for unnecessary inner-unsafe when marked on fn.
    // This check only applies to "pub" functions which are all exposed via C
    // API.
    clippy::not_unsafe_ptr_arg_deref,
)]

pub mod error;
pub mod runtime;
pub mod schema;
pub mod table;
use std::collections::HashMap;

use runtime::Runtime;

#[repr(C)]
pub struct MapRef {
    data: *const MapElementRef,
    size: libc::size_t,
}

#[repr(C)]
pub struct SerializedBuffer {
    data: *const u8,
    size: libc::size_t,
    offset: libc::size_t,
}

pub struct Map {
    data: HashMap<String, String>,
    disable_free: bool,
}

#[no_mangle]
pub extern "C" fn map_new(runtime: *const Runtime, capacity: usize) -> *const Map {
    let rt = unsafe { &*runtime };
    Box::into_raw(Box::new(rt.allocate_map(capacity)))
}

#[no_mangle]
pub extern "C" fn map_add(
    map: *mut Map,
    key: *const ByteArrayRef,
    value: *const ByteArrayRef,
) -> bool {
    if map.is_null() {
        return false;
    }

    let (key, value) = unsafe { ((&*key), (&*value)) };
    let map = unsafe { &mut *map };

    map.data.insert(key.to_string(), value.to_string());
    true
}

#[repr(C)]
pub struct MapElementRef {
    key: *const ByteArrayRef,
    value: *const ByteArrayRef,
}

#[repr(C)]
pub struct ByteArrayRef {
    data: *const u8,
    size: libc::size_t,
}

impl ByteArrayRef {
    fn from_str(s: &str) -> ByteArrayRef {
        ByteArrayRef {
            data: s.as_ptr(),
            size: s.len(),
        }
    }

    fn from_string(s: &String) -> ByteArrayRef {
        ByteArrayRef {
            data: s.as_ptr(),
            size: s.len(),
        }
    }

    fn to_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
    }

    #[allow(clippy::mut_from_ref)]
    fn to_slice_mut(&self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.data as *mut u8, self.size) }
    }

    fn to_vec(&self) -> Vec<u8> {
        self.to_slice().to_vec()
    }

    fn to_str(&self) -> &str {
        // Trust caller to send UTF8. Even if we did do a checked call here with
        // error, the caller can still have a bad pointer or something else
        // wrong. Therefore we trust the caller implicitly.
        unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.data, self.size)) }
    }

    fn to_string(&self) -> String {
        self.to_str().to_string()
    }

    #[allow(dead_code)]
    fn to_option_slice(&self) -> Option<&[u8]> {
        if self.size == 0 {
            None
        } else {
            Some(self.to_slice())
        }
    }

    fn to_option_vec(&self) -> Option<Vec<u8>> {
        if self.size == 0 {
            None
        } else {
            Some(self.to_vec())
        }
    }

    fn to_option_str(&self) -> Option<&str> {
        if self.size == 0 {
            None
        } else {
            Some(self.to_str())
        }
    }

    fn to_option_string(&self) -> Option<String> {
        self.to_option_str().map(str::to_string)
    }

    fn to_str_map_on_newlines(&self) -> HashMap<&str, &str> {
        let strs: Vec<&str> = self.to_str().split('\n').collect();
        strs.chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect()
    }

    fn to_string_map_on_newlines(&self) -> HashMap<String, String> {
        self.to_str_map_on_newlines()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

/// Metadata is <key1>\n<value1>\n<key2>\n<value2>. Metadata keys or
/// values cannot contain a newline within.
type MetadataRef = ByteArrayRef;

#[repr(C)]
pub struct DynamicArray {
    data: *const ByteArray,
    size: libc::size_t,
    /// For internal use only.
    cap: libc::size_t,
    /// For internal use only.
    disable_free: bool,
}

impl DynamicArray {
    pub(crate) fn from_vec_string(input: Vec<String>) -> Self {
        let data: Vec<ByteArray> = input
            .into_iter()
            .map(|path| ByteArray::from_utf8(path.to_string()))
            .collect();
        let dyn_array = DynamicArray {
            data: data.as_ptr(),
            size: data.len(),
            cap: data.capacity(),
            disable_free: false,
        };
        dyn_array
    }
}
#[repr(C)]
pub struct ByteArray {
    data: *const u8,
    size: libc::size_t,
    /// For internal use only.
    cap: libc::size_t,
    /// For internal use only.
    disable_free: bool,
}

impl ByteArray {
    fn from_utf8(str: String) -> ByteArray {
        ByteArray::from_vec(str.into_bytes())
    }

    fn from_vec(vec: Vec<u8>) -> ByteArray {
        // Mimics Vec::into_raw_parts that's only available in nightly
        let mut vec = std::mem::ManuallyDrop::new(vec);
        ByteArray {
            data: vec.as_mut_ptr(),
            size: vec.len(),
            cap: vec.capacity(),
            disable_free: false,
        }
    }

    #[allow(dead_code)]
    fn from_vec_disable_free(vec: Vec<u8>) -> ByteArray {
        let mut b = ByteArray::from_vec(vec);
        b.disable_free = true;
        b
    }

    fn into_raw(self) -> *mut ByteArray {
        Box::into_raw(Box::new(self))
    }
}

// Required because these instances are used by lazy_static and raw pointers are
// not usually safe for send/sync.
unsafe impl Send for ByteArray {}
unsafe impl Sync for ByteArray {}

impl Drop for ByteArray {
    fn drop(&mut self) {
        // In cases where freeing is disabled (or technically some other
        // drop-but-not-freed situation though we don't expect any), the bytes
        // remain non-null so we re-own them here. See "byte_array_free" in
        // runtime.rs.
        if !self.data.is_null() {
            unsafe { Vec::from_raw_parts(self.data as *mut u8, self.size, self.cap) };
        }
    }
}

pub struct CancellationToken {
    token: tokio_util::sync::CancellationToken,
}

#[no_mangle]
pub extern "C" fn cancellation_token_new() -> *mut CancellationToken {
    Box::into_raw(Box::new(CancellationToken {
        token: tokio_util::sync::CancellationToken::new(),
    }))
}

#[no_mangle]
pub extern "C" fn cancellation_token_cancel(token: *mut CancellationToken) {
    let token = unsafe { &*token };
    token.token.cancel();
}

#[no_mangle]
pub extern "C" fn cancellation_token_free(token: *mut CancellationToken) {
    unsafe {
        let _ = Box::from_raw(token);
    }
}