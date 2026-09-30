//! **Los aparatos que no hay** (tanda 16 de Cyberpunk, 30-09): lo que el
//! juego importa para buscar mandos y aparatos USB (HID, SETUPAPI,
//! CFGMGR32) y LDAP (WLDAP32, de libcurl). DURAS del censo: sin ellas no
//! arranca; con ellas, cada busqueda contesta lo que Windows diria en una
//! maquina SIN esos aparatos.
//!
//! ```text
//!    HID         HidD_GetHidGuid (el GUID de la clase HID, el de verdad);
//!                lo demas pide un handle de aparato que nadie da: FALSE, o
//!                HIDP_STATUS_INVALID_PREPARSED_DATA
//!    SETUPAPI    SetupDiGetClassDevsW da una lista VACIA; enumerarla,
//!                ERROR_NO_MORE_ITEMS
//!    CFGMGR32    el nodo raiz existe; la lista de aparatos esta vacia; sus
//!                propiedades, CR_NO_SUCH_VALUE; CM_MapCrToWin32Err traduce
//!    WLDAP32     por ORDINAL, y la casa no tiene LDAP: todas a una que lo
//!                dice por la consola y devuelve 0 (libcurl solo la llama
//!                con una direccion ldap://, y el juego no tiene ninguna)
//! ```

use crate::{aviso, dir, kernel32};

const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NO_MORE_ITEMS: u32 = 259;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const HIDP_STATUS_INVALID_PREPARSED_DATA: u32 = 0xC011_0001;
const CR_SUCCESS: u32 = 0;
const CR_INVALID_POINTER: u32 = 0x03;
const CR_NO_SUCH_VALUE: u32 = 0x25;
const CR_BUFFER_SMALL: u32 = 0x1A;

// -- HID -------------------------------------------------------------------------------

/// `HidD_GetHidGuid`: {4D1E55B2-F16F-11CF-88CB-001111000030}.
extern "win64" fn hidd_get_hid_guid(g: *mut u8) {
    if g.is_null() {
        return;
    }
    let v: [u8; 16] = [0xB2, 0x55, 0x1E, 0x4D, 0x6F, 0xF1, 0xCF, 0x11, 0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30];
    // SAFETY: un GUID del `.exe`.
    unsafe { core::ptr::copy_nonoverlapping(v.as_ptr(), g, 16) };
}

/// Las HidD_* con un handle de aparato: nadie los da, FALSE.
extern "win64" fn hidd_sin_aparato(_h: u64, _a: u64, _b: u64) -> i32 {
    kernel32::poner_error(ERROR_INVALID_HANDLE);
    0
}

/// Las HidP_* con datos del aparato: no los hay.
extern "win64" fn hidp_sin_datos(_a: u64, _b: u64, _c: u64, _d: u64) -> u32 {
    HIDP_STATUS_INVALID_PREPARSED_DATA
}

// -- SETUPAPI ----------------------------------------------------------------------------

/// El HDEVINFO de una lista vacia: uno fijo, no es un puntero.
const LISTA: u64 = 0x5A1E_D000;

extern "win64" fn setup_di_get_class_devs_w(_g: *const u8, _e: *const u16, _h: u64, _f: u32) -> u64 {
    LISTA
}

extern "win64" fn setup_di_destroy_device_info_list(h: u64) -> i32 {
    if h != LISTA {
        kernel32::poner_error(ERROR_INVALID_HANDLE);
        return 0;
    }
    1
}

/// Enumerar la lista vacia: ERROR_NO_MORE_ITEMS desde el primero.
extern "win64" fn setup_di_nada_mas(h: u64, _a: u64, _b: u64, _c: u64, _d: u64) -> i32 {
    kernel32::poner_error(if h == LISTA { ERROR_NO_MORE_ITEMS } else { ERROR_INVALID_HANDLE });
    0
}

/// Detalles o propiedades de un aparato que no se enumero: no hay.
#[allow(clippy::too_many_arguments)]
extern "win64" fn setup_di_sin_aparato(_h: u64, _a: u64, _b: u64, _c: u64, _d: u64, _e: u64, _f: u64) -> i32 {
    kernel32::poner_error(ERROR_INVALID_PARAMETER);
    0
}

extern "win64" fn setup_get_inf_driver_store_location_w(_a: u64, _b: u64, _c: u64, _d: u64, _e: u32, _f: u64) -> i32 {
    kernel32::poner_error(ERROR_FILE_NOT_FOUND);
    0
}

// -- CFGMGR32 -----------------------------------------------------------------------------

/// `CM_Locate_DevNodeW`: el nodo raiz (sin nombre) existe; los demas, no.
extern "win64" fn cm_locate_dev_node_w(dn: *mut u32, id: *const u16, _f: u32) -> u32 {
    if dn.is_null() {
        return CR_INVALID_POINTER;
    }
    // SAFETY: un DEVINST del `.exe`; el nombre, una cadena suya.
    unsafe {
        if !id.is_null() && id.read() != 0 {
            dn.write(0);
            return 0x0D; // CR_NO_SUCH_DEVNODE
        }
        dn.write(1);
    }
    CR_SUCCESS
}

/// `CM_Get_Device_ID_List_SizeW`: la lista vacia son dos ceros (1
/// caracter: la lista vacia acaba en uno).
extern "win64" fn cm_get_device_id_list_size_w(n: *mut u32, _filtro: *const u16, _f: u32) -> u32 {
    if n.is_null() {
        return CR_INVALID_POINTER;
    }
    // SAFETY: un ULONG del `.exe`.
    unsafe { n.write(1) };
    CR_SUCCESS
}

extern "win64" fn cm_get_device_id_list_w(_filtro: *const u16, b: *mut u16, n: u32, _f: u32) -> u32 {
    if b.is_null() {
        return CR_INVALID_POINTER;
    }
    if n < 1 {
        return CR_BUFFER_SMALL;
    }
    // SAFETY: `n` caracteres del `.exe`.
    unsafe { b.write(0) };
    CR_SUCCESS
}

#[allow(clippy::too_many_arguments)]
extern "win64" fn cm_get_dev_node_property_w(_dn: u32, _clave: u64, _tipo: u64, _b: u64, _n: u64, _f: u32) -> u32 {
    CR_NO_SUCH_VALUE
}

/// `CM_MapCrToWin32Err(cr, por_defecto)`.
extern "win64" fn cm_map_cr_to_win32_err(cr: u32, por_defecto: u32) -> u32 {
    match cr {
        0x00 => 0,
        0x02 => 8,                   // CR_OUT_OF_MEMORY -> ERROR_NOT_ENOUGH_MEMORY
        0x03 => 1784,                // CR_INVALID_POINTER -> ERROR_INVALID_USER_BUFFER
        0x05 | 0x1D => 87,           // CR_INVALID_FLAG, CR_INVALID_DEVICE_ID -> PARAMETER
        0x0D | 0x25 | 0x13 => 1168,  // NO_SUCH_DEVNODE, NO_SUCH_VALUE, NO_SUCH_REGISTRY_KEY -> ERROR_NOT_FOUND
        0x1A => 122,                 // CR_BUFFER_SMALL -> ERROR_INSUFFICIENT_BUFFER
        0x1F => 13,                  // CR_INVALID_DATA -> ERROR_INVALID_DATA
        0x33 => 5,                   // CR_ACCESS_DENIED -> ERROR_ACCESS_DENIED
        _ => por_defecto,
    }
}

// -- WLDAP32 ------------------------------------------------------------------------------

/// Cualquier funcion de WLDAP32: la casa no tiene LDAP.
extern "win64" fn sin_ldap() -> u64 {
    aviso("WLDAP32: la casa no tiene LDAP");
    0
}

/// WLDAP32 por ordinal: todas a `sin_ldap`.
pub(crate) fn ldap_por_ordinal(_o: u16) -> Option<u64> {
    Some(dir!(sin_ldap))
}

pub(crate) fn buscar(n: &str) -> Option<u64> {
    Some(match n {
        "HidD_GetHidGuid" => dir!(hidd_get_hid_guid),
        "HidD_GetAttributes" | "HidD_GetPreparsedData" | "HidD_FreePreparsedData" | "HidD_GetFeature" | "HidD_SetFeature" | "HidD_GetSerialNumberString" | "HidD_GetManufacturerString" | "HidD_GetProductString" => dir!(hidd_sin_aparato),
        "HidP_GetCaps" | "HidP_GetValueCaps" | "HidP_GetButtonCaps" => dir!(hidp_sin_datos),
        "SetupDiGetClassDevsW" | "SetupDiGetClassDevsA" => dir!(setup_di_get_class_devs_w),
        "SetupDiDestroyDeviceInfoList" => dir!(setup_di_destroy_device_info_list),
        "SetupDiEnumDeviceInterfaces" | "SetupDiEnumDeviceInfo" => dir!(setup_di_nada_mas),
        "SetupDiGetDeviceInterfaceDetailW" | "SetupDiGetDeviceRegistryPropertyW" => dir!(setup_di_sin_aparato),
        "SetupGetInfDriverStoreLocationW" => dir!(setup_get_inf_driver_store_location_w),
        "CM_Locate_DevNodeW" => dir!(cm_locate_dev_node_w),
        "CM_Get_Device_ID_List_SizeW" => dir!(cm_get_device_id_list_size_w),
        "CM_Get_Device_ID_ListW" => dir!(cm_get_device_id_list_w),
        "CM_Get_DevNode_PropertyW" => dir!(cm_get_dev_node_property_w),
        "CM_MapCrToWin32Err" => dir!(cm_map_cr_to_win32_err),
        n if n.starts_with("ldap_") || n.starts_with("Ldap") => dir!(sin_ldap),
        _ => return None,
    })
}
