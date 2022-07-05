use crate::mosquitto_dev::{mosquitto_broker_publish, mosquitto_property, mosquitto_message, mosquitto_get_retained};
use crate::Error;
use crate::MosquittoMessage;
use crate::{Success, QOS};
use crate::size_t;
use libc::c_void;
use std::convert::TryInto;
use std::ffi::CString;
use std::mem;
use std::os::raw::c_char;

/// Broadcast a message from the broker
/// If called in a username and password check the connecting client will not get the message
/// Use the publish_to_client combined with this if you want to send to all clients including the one that is connecting
pub fn publish_broadcast(
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
            nullptr as *const c_char, // client id to send to, null = all clients
            topic as *const c_char,
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
pub fn publish_to_client(
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
            client_id as *const c_char, // client id to send to, null = all clients
            topic as *const c_char,     // topic to publish on
            payload_len as i32,         // payload length in bytes, 0 for empty payload
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

pub fn get_retained<'a>(topic: &'a str, buf_size: usize) -> Result<Vec<MosquittoMessage>, String> {
    let cstr = &CString::new(topic).map_err(|_| {
        format!(
            "failed to make cstring for topic {} probably it contains a 0 byte",
            topic
        )
    })?;

    let topic_ptr = cstr.as_bytes_with_nul().as_ptr() as *const c_char;

    unsafe {
        let mut buffer = Vec::<*mut mosquitto_message>::new();
        for _ in 0..buf_size {
            let msg = Box::into_raw(Box::new(mosquitto_message {
                mid: 0,
                topic: std::ptr::null_mut::<c_char>(),
                payload: std::ptr::null_mut::<libc::c_void>(),
                payloadlen: 0,
                qos: 0,
                retain: false,
            }));
            buffer.push(msg);
        }
        let mut messages_found: u64 = 0;
        let mut len = buffer.len() as size_t;
        let rc = mosquitto_get_retained(
            topic_ptr,
            buffer.as_slice().as_ptr(),
            &mut len as *mut size_t,
            &mut messages_found as *mut size_t,
        );
        let i = crate::mosq_err_t_MOSQ_ERR_SUCCESS;
        match rc {
            crate::mosq_err_t_MOSQ_ERR_SUCCESS => {},
            crate::mosq_err_t_MOSQ_ERR_BUFFER_FULL => { return get_retained(topic, buf_size*2)},
            error => {
                let msg = match error {
                    crate::mosq_err_t_MOSQ_ERR_INVAL => {"MOSQ_ERR_INVAL".to_string()},
                    crate::mosq_err_t_MOSQ_ERR_NOMEM => {"MOSQ_ERR_NOMEM".to_string()},
                    crate::mosq_err_t_MOSQ_ERR_PROTOCOL => {"MOSQ_ERR_PROTOCOL".to_string()},
                    
                    _ => {
                        //todo!(implement for all known errors)
                        "UNKNOWN ERROR".to_string()}
                };
                let msg = format!("mosquitto_get_retained failed. return code from mosquitto: {}", msg);
                reclaim_into_box(buffer, buf_size, messages_found).map_err(|e| format!("failed to reclaim memory (potentially big problem): {e} while handling this error: {msg}"))?;
                return Err(format!("mosquitto_get_retained failed. return code from mosquitto: {}", msg))
            },
        }
        let messages = reclaim_into_box(buffer, buf_size, messages_found)?;
        convert_to_rust_type::<'a>(messages)
    }
}
unsafe fn reclaim_into_box(buffer: Vec<*mut mosquitto_message>, buf_size: usize, messages_found: u64) -> Result<Vec<mosquitto_message>, String> {
    let mut mosquitto_messages: Vec<mosquitto_message> = Vec::new();

    //put all pointers back in boxes
    for i in 0..buf_size {
        if (buffer[i as usize]).is_null() {
            return Err("Dereferencing a null pointer is a bad idea".to_string());
        }
        let msg = Box::from_raw(buffer[i as usize]);
        if i < messages_found as usize {
            mosquitto_messages.push(*msg);
        }
    }
    Ok(mosquitto_messages)
}
unsafe fn convert_to_rust_type<'a>(
    messages: Vec<mosquitto_message>,
) -> Result<Vec<MosquittoMessage<'a>>, String> {
    //populate result vec
    let mut result: Vec<MosquittoMessage> = Vec::new();
    for msg in messages {
        let topic = std::ffi::CStr::from_ptr(msg.topic).to_str().map_err(|_| {
            "get_retained failed to create topic &str from CStr pointer".to_string()
        })?;

        let payload: &[u8] =
            std::slice::from_raw_parts(msg.payload as *const u8, msg.payloadlen as usize);

        let message = MosquittoMessage {
            topic,
            payload,
            qos: msg.qos,
            retain: msg.retain,
        };
        result.push(message);
    }
    Ok(result)
}
