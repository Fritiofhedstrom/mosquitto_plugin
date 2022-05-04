use crate::mosquitto_dev::*;
use crate::Error;
use crate::{Success, QOS};
use libc::c_void;
use std::ffi::CString;

#[derive(Clone, Copy)]
pub struct MosquittoPublishCalls {}

impl MosquittoPublishCalls {
    /// Broadcast a message from the broker
    /// If called in a username and password check the connecting client will not get the message
    /// Use the broker_publish_to_client combined with this if you want to send to all clients including the one that is connecting
    pub fn broker_broadcast_publish(
        &self,
        topic: &str,
        payload: &[u8],
        qos: QOS,
        retain: bool,
    ) -> Result<Success, Error> {
        let cstr = &CString::new(topic).expect("no cstring for u");
        let bytes = cstr.as_bytes_with_nul();
        let topic = bytes.as_ptr();

        let nullptr: *const c_void = std::ptr::null();
        let properties: *mut mosquitto_property = std::ptr::null_mut();

        // let payload: *mut c_void = std::ptr::null_mut(); // payload bytes, non-null if payload length > 0, must be heap allocated
        let payload_len = payload.len();
        let payload: *const c_void = Box::new(payload).as_ptr() as *const c_void; // payload bytes, non-null if payload length > 0, must be heap allocated

        unsafe {
            let c_payload: *mut c_void =
                libc::malloc(std::mem::size_of::<u8>() * payload_len) as *mut c_void;
            payload.copy_to(c_payload, payload_len);
            /*
             * https://mosquitto.org/api2/files/mosquitto_broker-h.html#mosquitto_broker_publish
             * maybe want to switch to mosquitto_broker_publish to maintain ownership over
             * payload memory.
             * payload: payload bytes.  If payloadlen > 0 this must not be NULL.  Must be allocated on the heap.  Will be freed by mosquitto after use if the function returns success."
             * What happens if it is not successfull? Do i need to free the memory myself? This is a leak if if i dont' free memory  in all cases except 0 (Success) below?
             */
            let res = mosquitto_broker_publish(
                nullptr as *const i8, // client id to send to, null = all clients
                topic as *const i8,
                payload_len as i32, // payload length in bytes, 0 for empty payload
                c_payload, // payload bytes, non-null if payload length > 0, must be heap allocated
                qos.to_i32(), // qos
                retain,    // retain
                properties, //mqtt5 properties
            );
            match res {
                0 => Ok(Success),
                1 => Err(Error::NoMem),
                3 => Err(Error::Inval),
                _default => Err(Error::Unknown),
            }
        }
    }

    /// To be called from implementations of the plugin when
    /// a plugin wants to publish to a specific client.
    pub fn broker_publish_to_client(
        &self,
        client_id: &str,
        topic: &str,
        payload: &[u8],
        qos: QOS,
        retain: bool,
    ) -> Result<Success, Error> {
        let cstr = &CString::new(client_id).expect("no cstring for u");
        let bytes = cstr.as_bytes_with_nul();
        let client_id = bytes.as_ptr();

        let cstr = &CString::new(topic).expect("no cstring for u");
        let bytes = cstr.as_bytes_with_nul();
        let topic = bytes.as_ptr();

        let payload_len = payload.len();
        let payload: *const c_void = Box::new(payload).as_ptr() as *const c_void;

        unsafe {
            let c_payload: *mut c_void =
                libc::malloc(std::mem::size_of::<u8>() * payload_len) as *mut c_void;
            payload.copy_to(c_payload, payload_len);

            let res = mosquitto_broker_publish(
                client_id as *const i8, // client id to send to, null = all clients
                topic as *const i8,     // topic to publish on
                payload_len as i32,     // payload length in bytes, 0 for empty payload
                c_payload, // payload bytes, non-null if payload length > 0, must be heap allocated
                qos.to_i32(), // qos
                retain,    // retain
                std::ptr::null_mut(), //mqtt5 properties
            );
            match res {
                0 => Ok(Success),
                1 => Err(Error::NoMem),
                3 => Err(Error::Inval),
                _default => Err(Error::Unknown),
            }
        }
    }
}
