// Copy of https://gitlab.com/surfer-project/surfer/-/blob/main/libsurfer/src/wcp/proto.rs
// with implemented Serialize for timestamp
use num::ToPrimitive;
use num::{BigInt, FromPrimitive};
use serde::{Deserialize, Deserializer, Serialize, de};
use serde_json::Number;

/// A reference to a currently displayed item. From the protocol perspective,
/// This can be any integer or a string and what it is is decided by the server,
/// in this case surfer.
/// Since the representation is up to the server, clients cannot generate these on its
/// own, it can only use the ones it has received from the server.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy)]
#[serde(transparent)]
pub struct DisplayedItemRef(pub usize);

impl From<&DisplayedItemRef> for DisplayedItemRef {
    fn from(value: &DisplayedItemRef) -> Self {
        DisplayedItemRef(value.0)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ItemInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub t: String,
    pub id: DisplayedItemRef,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MarkerInfo {
    #[serde(
        deserialize_with = "deserialize_timestamp",
        serialize_with = "serialize_timestamp"
    )]
    pub time: BigInt,
    pub name: Option<String>,
    pub move_focus: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command")]
#[allow(non_camel_case_types)]
pub enum WcpResponse {
    get_item_list { ids: Vec<DisplayedItemRef> },
    get_item_info { results: Vec<ItemInfo> },
    add_variables { ids: Vec<DisplayedItemRef> },
    add_scope { ids: Vec<DisplayedItemRef> },
    add_markers { ids: Vec<DisplayedItemRef> },
    ack,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "event")]
#[allow(non_camel_case_types)]
pub enum WcpEvent {
    waveforms_loaded { source: String },
    goto_declaration { variable: String },
    add_drivers { variable: String },
    add_loads { variable: String },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum WcpSCMessage {
    greeting {
        version: String,
        commands: Vec<String>,
    },
    response(WcpResponse),
    error {
        error: String,
        arguments: Vec<String>,
        message: String,
    },
    event(WcpEvent),
}

impl WcpSCMessage {
    pub fn create_greeting(version: usize, commands: Vec<String>) -> Self {
        Self::greeting {
            version: version.to_string(),
            commands,
        }
    }

    pub fn create_error(error: String, arguments: Vec<String>, message: String) -> Self {
        Self::error {
            error,
            arguments,
            message,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "command")]
#[allow(non_camel_case_types)]
pub enum WcpCommand {
    /// Responds with [WcpResponse::get_item_list] which contains a list of the items
    /// in the currently loaded waveforms
    get_item_list,
    /// Responds with [WcpResponse::get_item_info] which contains information about
    /// each item specified in `ids` in the same order as in the `ids` array.
    /// Responds with an error if any of the specified IDs are not items in the currently loaded
    /// waveform.
    get_item_info { ids: Vec<DisplayedItemRef> },
    /// Changes the color of the specified item to the specified color.
    /// Responds with [WcpResponse::ack]
    /// Responds with an error if the `id` does not exist in the currently loaded waveform.
    set_item_color { id: DisplayedItemRef, color: String },
    /// Adds the specified variables to the view.
    /// Responds with [WcpResponse::add_variables] which contains a list of the item references
    /// that can be used to reference the added items later
    /// Responds with an error if no waveforms are loaded
    add_variables { variables: Vec<String> },
    /// Adds all variables in the specified scope to the view.
    /// Does so recursively if specified
    /// Responds with [WcpResponse::add_variables] which contains a list of the item references
    /// that can be used to reference the added items later
    /// Responds with an error if no waveforms are loaded
    add_scope {
        scope: String,
        #[serde(default)]
        recursive: bool,
    },
    /// Adds the specified markers to the view.
    /// TODO response
    add_markers { markers: Vec<MarkerInfo> },
    /// Reloads the waveform from disk if this is possible for the current waveform format.
    /// If it is not possible, this has no effect.
    /// Responds instantly with [WcpResponse::ack]
    /// Once the waveforms have been loaded, a separate event is triggered
    reload,
    /// Moves the viewport to center it on the specified timestamp. Does not affect the zoom
    /// level.
    /// Responds with [WcpResponse::ack]
    set_viewport_to {
        #[serde(
            deserialize_with = "deserialize_timestamp",
            serialize_with = "serialize_timestamp"
        )]
        timestamp: BigInt,
    },
    /// Moves the viewport to center it on the specified timestamps range. Does affect the zoom
    /// level.
    /// Responds with [WcpResponse::ack]
    set_viewport_range {
        #[serde(
            deserialize_with = "deserialize_timestamp",
            serialize_with = "serialize_timestamp"
        )]
        start: BigInt,
        #[serde(
            deserialize_with = "deserialize_timestamp",
            serialize_with = "serialize_timestamp"
        )]
        end: BigInt,
    },
    /// Removes the specified items from the view.
    /// Responds with [WcpResponse::ack]
    /// Does not error if some of the IDs do not exist
    remove_items { ids: Vec<DisplayedItemRef> },
    /// Sets the specified ID as the _focused_ item.
    /// Responds with [WcpResponse::ack]
    /// Responds with an error if no waveforms are loaded or if the item reference
    /// does not exist
    // FIXME: What does this mean in the context of the protocol in general, feels kind
    // of like a Surfer specific thing. Do we have a use case for it
    focus_item { id: DisplayedItemRef },
    /// Removes all currently displayed items
    /// Responds with [WcpResponse::ack]
    clear,
    /// Loads a waveform from the specified file.
    /// Responds instantly with [WcpResponse::ack]
    /// Once the file is loaded, a [WcpEvent::waveforms_loaded] is emitted.
    load { source: String },
    /// Zooms out fully to fit the whole waveform in the view
    /// Responds instantly with [WcpResponse::ack]
    zoom_to_fit { viewport_idx: usize },
    /// Shut down the WCP server.
    // FIXME: What does this mean? Does it kill the server, the current connection or surfer itself?
    shutdowmn,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
pub enum WcpCSMessage {
    #[serde(rename = "greeting")]
    greeting {
        version: String,
        commands: Vec<String>,
    },
    command(WcpCommand),
}

impl WcpCSMessage {
    pub fn create_greeting(version: usize, commands: Vec<String>) -> Self {
        Self::greeting {
            version: version.to_string(),
            commands,
        }
    }
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<BigInt, D::Error>
where
    D: Deserializer<'de>,
{
    let num = Number::deserialize(deserializer)?;
    if let Some(timestamp) = num.as_u128() {
        Ok(BigInt::from(timestamp))
    } else if let Some(timestamp) = num.as_i128() {
        Ok(BigInt::from(timestamp))
    } else if let Some(timestamp) = num.as_f64() {
        BigInt::from_f64(timestamp).ok_or_else(|| {
            <D::Error as serde::de::Error>::invalid_value(
                serde::de::Unexpected::Float(timestamp),
                &"a finite value",
            )
        })
    } else {
        Err(de::Error::custom(
            "Error durian deserialization of timestamp value {num}",
        ))
    }
}

fn serialize_timestamp<S>(timestamp: &BigInt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::Error as err;
    if let Some(timestamp) = timestamp.to_u128() {
        Number::from_u128(timestamp)
            .ok_or(err::custom("Conversion failed"))?
            .serialize(serializer)
    } else if let Some(timestamp) = timestamp.to_i128() {
        Number::from_i128(timestamp)
            .ok_or(err::custom("Conversion failed"))?
            .serialize(serializer)
    } else if let Some(timestamp) = timestamp.to_f64() {
        Number::from_f64(timestamp)
            .ok_or(err::custom("Conversion failed"))?
            .serialize(serializer)
    } else {
        Err(err::custom("Conversion failed"))
    }
}
