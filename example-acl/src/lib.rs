// Has to be included, to get the errors and success parameters that are used in the
// generate_dynamic_library macro invocation
use mosquitto_plugin::*;

// Some simple nonsense structure to use as an example
#[derive(Debug)]
pub struct Test {
    i: i32,
    s: String,
}

// Required trait implementation
impl MosquittoPlugin for Test {
    fn init(opts: std::collections::HashMap<&str, &str>) -> Self {
        // These are the strings provided after "auth_opt_<key> value" in the mosquitto.conf
        // only that they are provided on a hashmap form here
        let default = "hej";
        let topic = opts.get("topic").unwrap_or(&default);
        let level = opts.get("level").unwrap_or(&default);
        let level = level.parse().unwrap_or(0);
        Test {
            i: level,
            s: topic.to_string(),
        }
    }
    fn username_password(
        &mut self,
        client: &dyn MosquittoClientContext,
        u: Option<&str>,
        p: Option<&str>,
    ) -> Result<Success, Error> {
        let client_id = client.get_id();
        println!("USERNAME_PASSWORD({}) {:?} - {:?}", client_id, u, p);
        if u.is_none() || p.is_none() {
            return Err(Error::Auth);
        }
        let u = u.unwrap();
        let p = p.unwrap();
        // this will allow all username/password where the password is the username in reverse
        let rp: String = p.chars().rev().collect();
        if rp == u {
            // Declare the accepted new client
            mosquitto_calls::publish_broadcast(
                "new_client",
                "very_client is a friend. Lets make it feel at home!".as_bytes(),
                QOS::AtMostOnce,
                true,
            )?;
            mosquitto_calls::publish_broadcast(
                "test/hello",
                "very_client is a friend. Lets make it feel at home!".as_bytes(),
                QOS::AtMostOnce,
                true,
            )?;
            mosquitto_calls::publish_broadcast(
                "123/321/test",
                "very_client is a friend. Lets make it feel at home!".as_bytes(),
                QOS::AtMostOnce,
                true,
            )?;
            // Welcome the new client privately
            mosquitto_calls::publish_to_client(
                &client_id,
                "greeting",
                format!("Welcome {}", client_id).as_bytes(),
                QOS::AtMostOnce,
                true,
            )?;
            Ok(Success)
        } else {
            Err(Error::Auth)
        }
    }

    fn acl_check(
        &mut self,
        client: &dyn MosquittoClientContext,
        level: AclCheckAccessLevel,
        msg: MosquittoMessage,
    ) -> Result<Success, mosquitto_plugin::Error> {
        Ok(Success)
    }

    fn on_disconnect(&mut self, client: &dyn MosquittoClientContext, reason: i32) {
        println!(
            "Plugin on_disconnect, Client {} is disconnecting",
            client.get_id()
        );
    }

    fn on_message(&mut self, client: &dyn MosquittoClientContext, message: MosquittoMessage) {
        println!(
            "Plugin on_message: client {}: Topic: {}, Payload: {:?}",
            client.get_id(),
            message.topic,
            message.payload
        );
        let retained_database = mosquitto_calls::get_retained("#", 10);
        println!("retained_database: {:?}", retained_database);
    }
}

// This generates the dynamic c bindings functions that are exported and usable by mosquitto
create_dynamic_library!(Test);
