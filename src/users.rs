use libc::{getgrgid_r, getpwuid_r};
use std::ffi::CStr;
use std::mem;
use std::ptr;

pub fn get_user_name_by_uid(uid: u32) -> Option<String> {
    let mut pwd = unsafe { mem::zeroed() };
    let mut buf = vec![0u8; 1024];
    let mut result = ptr::null_mut();
    unsafe {
        if getpwuid_r(
            uid,
            &mut pwd,
            buf.as_mut_ptr() as *mut _,
            buf.len(),
            &mut result,
        ) == 0
            && !result.is_null()
        {
            return Some(CStr::from_ptr(pwd.pw_name).to_string_lossy().into_owned());
        }
    }
    None
}

pub fn get_group_name_by_gid(gid: u32) -> Option<String> {
    let mut grp = unsafe { mem::zeroed() };
    let mut buf = vec![0u8; 1024];
    let mut result = ptr::null_mut();
    unsafe {
        if getgrgid_r(
            gid,
            &mut grp,
            buf.as_mut_ptr() as *mut _,
            buf.len(),
            &mut result,
        ) == 0
            && !result.is_null()
        {
            return Some(CStr::from_ptr(grp.gr_name).to_string_lossy().into_owned());
        }
    }
    None
}
