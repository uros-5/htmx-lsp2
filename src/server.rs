use std::collections::HashMap;
use std::sync::RwLock;

use dashmap::DashMap;
use ropey::Rope;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
    CompletionResponse, CompletionTriggerKind, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams, Hover,
    HoverContents, HoverParams, HoverProviderCapability, InitializedParams, MarkupContent,
    MarkupKind, MessageType, ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    Url,
};
use tower_lsp::lsp_types::{InitializeParams, ServerInfo};
use tower_lsp::{lsp_types::InitializeResult, Client, LanguageServer};

use crate::init_hx::{init_hx_tags, init_hx_values, HxCompletion};
use crate::position::{get_position_from_lsp_completion, Position, QueryType};

#[derive(Debug)]
pub struct BackendHtmx {
    client: Client,
    document_map: DashMap<String, Rope>,
    hx_tags: Vec<HxCompletion>,
    hx_attribute_values: HashMap<String, Vec<HxCompletion>>,
    is_helix: RwLock<bool>,
}

impl BackendHtmx {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            document_map: DashMap::new(),
            hx_tags: init_hx_tags(),
            hx_attribute_values: init_hx_values(),
            is_helix: RwLock::new(false),
        }
    }
    async fn on_change(&self, params: TextDocumentItem) {
        let rope = ropey::Rope::from_str(&params.text);
        self.document_map
            .insert(params.uri.to_string(), rope.clone());
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for BackendHtmx {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(client_info) = params.client_info {
            if client_info.name == "helix" {
                if let Ok(mut w) = self.is_helix.write() {
                    *w = true;
                }
            }
        }
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "-".to_string(),
                        "\"".to_string(),
                        " ".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: Default::default(),
                    completion_item: None,
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: String::from("fakehtmx"),
                version: Some(String::from("0.0.1")),
            }),
            offset_encoding: None,
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "initialized!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
        })
        .await
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {}

    async fn did_close(&self, _: DidCloseTextDocumentParams) {}

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        if let Some(text) = params.content_changes.first_mut() {
            self.on_change(TextDocumentItem {
                uri: params.text_document.uri,
                text: std::mem::take(&mut text.text),
            })
            .await
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let can_complete = {
            matches!(
                params.context,
                Some(CompletionContext {
                    trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                    ..
                }) | Some(CompletionContext {
                    trigger_kind: CompletionTriggerKind::INVOKED,
                    ..
                })
            )
        };
        if !can_complete {
            let is_helix = self.is_helix.read().is_ok_and(|d| *d);
            if !is_helix {
                return Ok(None);
            }
        }

        let uri = &params.text_document_position.text_document.uri;
        let result = get_position_from_lsp_completion(
            &params.text_document_position,
            &self.document_map,
            uri.to_string(),
            QueryType::Completion,
        );
        if let Some(result) = result {
            match result {
                Position::AttributeName(name) => {
                    if name.starts_with("hx-") {
                        let completions = self.hx_tags.clone();
                        let mut ret = Vec::with_capacity(completions.len());
                        for item in completions {
                            ret.push(CompletionItem {
                                label: item.name.to_string(),
                                detail: Some(item.desc.to_string()),
                                kind: Some(CompletionItemKind::TEXT),
                                ..Default::default()
                            });
                        }
                        return Ok(Some(ret).map(CompletionResponse::Array));
                    }
                }
                Position::AttributeValue { name, .. } => {
                    if let Some(completions) = self.hx_attribute_values.get(&name) {
                        let mut ret = Vec::with_capacity(completions.len());
                        for item in completions {
                            ret.push(CompletionItem {
                                label: item.name.to_string(),
                                detail: Some(item.desc.to_string()),
                                kind: Some(CompletionItemKind::TEXT),
                                ..Default::default()
                            });
                        }
                        return Ok(Some(ret).map(CompletionResponse::Array));
                    }
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let result = get_position_from_lsp_completion(
            &params.text_document_position_params,
            &self.document_map,
            uri.to_string(),
            QueryType::Hover,
        );

        if let Some(result) = result {
            match result {
                Position::AttributeName(name) => {
                    if let Some(res) = self
                        .hx_tags
                        .iter()
                        .find(|x| x.name == name.replace("hx-", ""))
                        .cloned()
                    {
                        let markup_content = MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: res.desc,
                        };
                        let hover_contents = HoverContents::Markup(markup_content);
                        let hover = Hover {
                            contents: hover_contents,
                            range: None,
                        };
                        return Ok(Some(hover));
                    }
                }
                Position::AttributeValue { name, value } => {
                    if let Some(res) = self.hx_attribute_values.get(&name) {
                        if let Some(res) = res.iter().find(|x| x.name == value).cloned() {
                            let markup_content = MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: res.desc,
                            };
                            let hover_contents = HoverContents::Markup(markup_content);
                            let hover = Hover {
                                contents: hover_contents,
                                range: None,
                            };
                            return Ok(Some(hover));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

struct TextDocumentItem {
    uri: Url,
    text: String,
}
