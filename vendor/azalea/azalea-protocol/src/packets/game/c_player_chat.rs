use std::{
    io::{self, Cursor, Write},
    sync::LazyLock,
};

use azalea_buf::{AzBuf, AzBufVar, BufReadError};
use azalea_chat::{
    FormattedText,
    translatable_component::{PrimitiveOrComponent, TranslatableComponent},
};
use azalea_core::{
    bitset::BitSet,
    data_registry::{DataRegistryWithKey, ResolvableDataRegistry},
    registry_holder::{RegistryHolder, RegistryType, dimension_type::ChatTypeElement, nbt_entry_as},
};
use azalea_crypto::signing::MessageSignature;
use azalea_protocol_macros::ClientboundGamePacket;
use azalea_registry::{
    DataRegistryKey, Holder,
    data::{ChatKind, ChatKindKey},
    identifier::Identifier,
};
use simdnbt::owned::NbtCompound;
use uuid::Uuid;

#[derive(AzBuf, ClientboundGamePacket, Clone, Debug, PartialEq)]
pub struct ClientboundPlayerChat {
    #[var]
    pub global_index: u32,
    pub sender: Uuid,
    #[var]
    pub index: u32,
    pub signature: Option<MessageSignature>,
    pub body: PackedSignedMessageBody,
    pub unsigned_content: Option<FormattedText>,
    pub filter_mask: FilterMask,
    pub chat_type: ChatTypeBound,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct PackedSignedMessageBody {
    pub content: String,
    pub timestamp: u64,
    pub salt: u64,
    pub last_seen: PackedLastSeenMessages,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct PackedLastSeenMessages {
    pub entries: Vec<PackedMessageSignature>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PackedMessageSignature {
    Signature(Box<MessageSignature>),
    Id(u32),
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub enum FilterMask {
    PassThrough,
    FullyFiltered,
    PartiallyFiltered(BitSet),
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct ChatTypeBound {
    pub chat_type: Holder<ChatKind, DirectChatType>,
    pub name: FormattedText,
    pub target_name: Option<FormattedText>,
}

#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct DirectChatType {
    pub chat: ChatTypeDecoration,
    pub narration: ChatTypeDecoration,
}
#[derive(AzBuf, Clone, Debug, PartialEq)]
pub struct ChatTypeDecoration {
    pub translation_key: String,
    pub parameters: Vec<ChatTypeDecorationParameter>,
    pub style: NbtCompound,
}

#[derive(AzBuf, Clone, Copy, Debug, PartialEq)]
pub enum ChatTypeDecorationParameter {
    Sender = 0,
    Target = 1,
    Content = 2,
}
impl ChatTypeDecorationParameter {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sender => "sender",
            Self::Target => "target",
            Self::Content => "content",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MessageSignatureCache {
    pub entries: Vec<Option<MessageSignature>>,
}

pub static GUESSED_DEFAULT_REGISTRIES_FOR_CHAT: LazyLock<RegistryHolder> =
    LazyLock::new(|| RegistryHolder {
        extra: [(
            Identifier::new("chat_type"),
            RegistryType {
                map: ChatKindKey::ALL
                    .iter()
                    .map(|k| (k.clone().into_ident(), NbtCompound::new()))
                    .collect(),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    });

impl ClientboundPlayerChat {
    #[must_use]
    pub fn content(&self) -> FormattedText {
        self.unsigned_content
            .clone()
            .unwrap_or_else(|| FormattedText::from(self.body.content.clone()))
    }

    #[must_use]
    pub fn message(&self) -> FormattedText {
        self.message_using_registries(&GUESSED_DEFAULT_REGISTRIES_FOR_CHAT)
    }

    #[must_use]
    pub fn message_using_registries(&self, registries: &RegistryHolder) -> FormattedText {
        self.chat_type.decorate(self.content(), registries)
    }
}

impl ChatTypeBound {
    pub fn translation_key(&self, registries: &RegistryHolder) -> &str {
        match &self.chat_type {
            Holder::Reference(r) => r
                .key(registries)
                .map(|r| r.chat_translation_key())
                .unwrap_or("chat.type.text"),
            Holder::Direct(d) => d.chat.translation_key.as_str(),
        }
    }

    #[must_use]
    pub fn decorate(&self, content: FormattedText, registries: &RegistryHolder) -> FormattedText {
        let (translation_key, parameters) = self.decoration(registries);

        let mut has_content = false;
        let mut args = Vec::with_capacity(parameters.len().max(1));
        for parameter in &parameters {
            match parameter.as_str() {
                "sender" => args.push(PrimitiveOrComponent::FormattedText(self.name.clone())),
                "target" => args.push(PrimitiveOrComponent::FormattedText(
                    self.target_name.clone().unwrap_or_default(),
                )),
                "content" => {
                    has_content = true;
                    args.push(PrimitiveOrComponent::FormattedText(content.clone()));
                }
                _ => {}
            }
        }
        if !has_content {
            args.push(PrimitiveOrComponent::FormattedText(content));
        }

        FormattedText::Translatable(TranslatableComponent::new(translation_key, args))
    }

    #[must_use]
    pub fn debug_source(&self, registries: &RegistryHolder) -> String {
        match &self.chat_type {
            Holder::Direct(d) => format!("Direct({})", d.chat.translation_key),
            Holder::Reference(r) => match r.key(registries) {
                Some(k) => format!("Reference({})", k.chat_translation_key()),
                None => "Reference(unresolved)".to_string(),
            },
        }
    }

    fn decoration(&self, registries: &RegistryHolder) -> (String, Vec<String>) {
        match &self.chat_type {
            Holder::Direct(d) => (
                d.chat.translation_key.clone(),
                d.chat.parameters.iter().map(|p| p.as_str().to_string()).collect(),
            ),
            Holder::Reference(r) => {
                if let Some((_, entry)) = r.resolve(registries)
                    && let Some(element) = nbt_entry_as::<ChatTypeElement>(entry)
                {
                    return (element.chat.translation_key, element.chat.parameters);
                }
                let (translation_key, parameters) = r
                    .key(registries)
                    .map(|k| (k.clone().chat_translation_key(), k.chat_translation_parameters()))
                    .unwrap_or(("chat.type.text", &["sender", "content"]));
                (
                    translation_key.to_string(),
                    parameters.iter().map(|s| (*s).to_string()).collect(),
                )
            }
        }
    }
}

impl AzBuf for PackedMessageSignature {
    fn azalea_read(buf: &mut Cursor<&[u8]>) -> Result<Self, BufReadError> {
        let id = u32::azalea_read_var(buf)?;
        if id == 0 {
            let full_signature = MessageSignature::azalea_read(buf)?;

            Ok(PackedMessageSignature::Signature(Box::new(full_signature)))
        } else {
            Ok(PackedMessageSignature::Id(id - 1))
        }
    }
    fn azalea_write(&self, buf: &mut impl Write) -> io::Result<()> {
        match self {
            PackedMessageSignature::Signature(full_signature) => {
                0u32.azalea_write_var(buf)?;
                full_signature.azalea_write(buf)?;
            }
            PackedMessageSignature::Id(id) => {
                (id + 1).azalea_write_var(buf)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod decorate_tests {
    use azalea_registry::DataRegistry;
    use simdnbt::owned::NbtTag;

    use super::*;

    fn bound(
        name: &str,
        target_name: Option<&str>,
        translation_key: &str,
        parameters: &[ChatTypeDecorationParameter],
    ) -> ChatTypeBound {
        ChatTypeBound {
            chat_type: Holder::Direct(DirectChatType {
                chat: ChatTypeDecoration {
                    translation_key: translation_key.to_string(),
                    parameters: parameters.to_vec(),
                    style: NbtCompound::new(),
                },
                narration: ChatTypeDecoration {
                    translation_key: "chat.type.text.narrate".to_string(),
                    parameters: vec![
                        ChatTypeDecorationParameter::Sender,
                        ChatTypeDecorationParameter::Content,
                    ],
                    style: NbtCompound::new(),
                },
            }),
            name: FormattedText::from(name.to_string()),
            target_name: target_name.map(|t| FormattedText::from(t.to_string())),
        }
    }

    fn rendered_args(text: &FormattedText) -> Vec<String> {
        let FormattedText::Translatable(component) = text else {
            panic!("expected a translatable component, got {text:?}");
        };
        component
            .args
            .iter()
            .map(|arg| {
                let PrimitiveOrComponent::FormattedText(t) = arg else {
                    panic!("expected a FormattedText arg, got {arg:?}");
                };
                t.to_string()
            })
            .collect()
    }

    #[test]
    fn plain_chat_is_sender_then_content() {
        let chat_type = bound(
            "steve",
            None,
            "chat.type.text",
            &[
                ChatTypeDecorationParameter::Sender,
                ChatTypeDecorationParameter::Content,
            ],
        );
        let text = chat_type.decorate(
            FormattedText::from("hi".to_string()),
            &GUESSED_DEFAULT_REGISTRIES_FOR_CHAT,
        );
        assert_eq!(rendered_args(&text), vec!["steve", "hi"]);
    }

    #[test]
    fn outgoing_whisper_is_target_then_content_with_no_sender() {
        let chat_type = bound(
            "steve",
            Some("alex"),
            "commands.message.display.outgoing",
            &[
                ChatTypeDecorationParameter::Target,
                ChatTypeDecorationParameter::Content,
            ],
        );
        let text = chat_type.decorate(
            FormattedText::from("hi".to_string()),
            &GUESSED_DEFAULT_REGISTRIES_FOR_CHAT,
        );
        assert_eq!(rendered_args(&text), vec!["alex", "hi"]);
    }

    #[test]
    fn team_message_is_target_sender_then_content() {
        let chat_type = bound(
            "steve",
            Some("Red Team"),
            "chat.type.team.text",
            &[
                ChatTypeDecorationParameter::Target,
                ChatTypeDecorationParameter::Sender,
                ChatTypeDecorationParameter::Content,
            ],
        );
        let text = chat_type.decorate(
            FormattedText::from("hi".to_string()),
            &GUESSED_DEFAULT_REGISTRIES_FOR_CHAT,
        );
        assert_eq!(rendered_args(&text), vec!["Red Team", "steve", "hi"]);
    }

    #[test]
    fn a_decoration_missing_content_still_carries_the_message() {
        let chat_type = bound("steve", None, "some.custom.key", &[
            ChatTypeDecorationParameter::Sender,
        ]);
        let text = chat_type.decorate(
            FormattedText::from("hi".to_string()),
            &GUESSED_DEFAULT_REGISTRIES_FOR_CHAT,
        );
        assert_eq!(rendered_args(&text), vec!["steve", "hi"]);
    }

    #[test]
    fn reference_without_registry_data_falls_back_to_the_bootstrap_order() {
        let chat_type = ChatTypeBound {
            chat_type: Holder::Reference(ChatKind::new_raw(3)),
            name: FormattedText::from("steve".to_string()),
            target_name: Some(FormattedText::from("alex".to_string())),
        };
        let text = chat_type.decorate(
            FormattedText::from("hi".to_string()),
            &GUESSED_DEFAULT_REGISTRIES_FOR_CHAT,
        );
        assert_eq!(rendered_args(&text), vec!["alex", "hi"]);
    }

    #[test]
    fn reference_with_registry_data_uses_the_servers_own_parameters() {
        let mut chat = NbtCompound::new();
        chat.insert("translation_key", "custom.whisper.key");
        chat.insert(
            "parameters",
            vec!["target".to_string(), "content".to_string()],
        );
        let mut narration = NbtCompound::new();
        narration.insert("translation_key", "chat.type.text.narrate");
        narration.insert(
            "parameters",
            vec!["sender".to_string(), "content".to_string()],
        );
        let mut entry = NbtCompound::new();
        entry.insert("chat", NbtTag::Compound(chat));
        entry.insert("narration", NbtTag::Compound(narration));

        let mut registries = RegistryHolder::default();
        registries.extra.insert(
            Identifier::new("chat_type"),
            RegistryType {
                map: [(Identifier::new("custom:whisper"), entry)].into_iter().collect(),
            },
        );

        let chat_type = ChatTypeBound {
            chat_type: Holder::Reference(ChatKind::new_raw(0)),
            name: FormattedText::from("steve".to_string()),
            target_name: Some(FormattedText::from("alex".to_string())),
        };
        let text = chat_type.decorate(FormattedText::from("hi".to_string()), &registries);
        let FormattedText::Translatable(component) = &text else {
            panic!("expected translatable");
        };
        assert_eq!(component.key, "custom.whisper.key");
        assert_eq!(rendered_args(&text), vec!["alex", "hi"]);
    }
}
