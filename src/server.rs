use std::collections::HashMap;
use std::sync::RwLock;

use dashmap::DashMap;
use ropey::Rope;
use serde::{Deserialize, Serialize};
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

use crate::tree_sitter_htmx::{get_position_from_lsp_completion, Position};

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
            hx_tags: create_basic_compls(),
            hx_attribute_values: create_basic_values(),
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

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
    }

    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file closed!")
            .await;
    }

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
                Position::AttributeValue { name, value } => {
                    let len = self.hx_attribute_values.len();
                    let msg = format!("{}, {}, {}", &name, &value, len);
                    self.client.log_message(MessageType::INFO, msg).await;

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

                    //
                }
            }
        }
        self.client.log_message(MessageType::INFO, "nista :(").await;
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let result = get_position_from_lsp_completion(
            &params.text_document_position_params,
            &self.document_map,
            uri.to_string(),
        );

        if let Some(result) = result {
            match result {
                Position::AttributeName(name) => {
                    self.client
                        .log_message(MessageType::INFO, "attribute name!")
                        .await;

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HxCompletion {
    pub name: String,
    pub desc: String,
}

pub fn create_basic_compls() -> Vec<HxCompletion> {
    let values = vec![
        ("boost", include_str!("./md/attributes/hx-boost.md")),
        ("delete", include_str!("./md/attributes/hx-delete.md")),
        ("get", include_str!("./md/attributes/hx-get.md")),
        ("include", include_str!("./md/attributes/hx-include.md")),
        ("patch", include_str!("./md/attributes/hx-patch.md")),
        ("post", include_str!("./md/attributes/hx-post.md")),
        ("put", include_str!("./md/attributes/hx-put.md")),
        ("swap", include_str!("./md/attributes/hx-swap.md")),
        ("target", include_str!("./md/attributes/hx-target.md")),
        ("trigger", include_str!("./md/attributes/hx-trigger.md")),
        ("vals", include_str!("./md/attributes/hx-vals.md")),
        (
            "hx-push-url",
            include_str!("./md/attributes/hx-push-url.md"),
        ),
        ("select", include_str!("./md/attributes/hx-select.md")),
        ("ext", include_str!("./md/attributes/hx-ext.md")),
        ("on", include_str!("./md/attributes/hx-on.md")),
        (
            "hx-select-oob",
            include_str!("./md/attributes/hx-select-oob.md"),
        ),
        (
            "hx-swap-oob",
            include_str!("./md/attributes/hx-swap-oob.md"),
        ),
        ("confirm", include_str!("./md/attributes/hx-confirm.md")),
        ("disable", include_str!("./md/attributes/hx-disable.md")),
        (
            "hx-encoding",
            include_str!("./md/attributes/hx-encoding.md"),
        ),
        ("headers", include_str!("./md/attributes/hx-headers.md")),
        ("history", include_str!("./md/attributes/hx-history.md")),
        (
            "hx-history-elt",
            include_str!("./md/attributes/hx-history-elt.md"),
        ),
        (
            "hx-indicator",
            include_str!("./md/attributes/hx-indicator.md"),
        ),
        ("params", include_str!("./md/attributes/hx-params.md")),
        (
            "hx-preserve",
            include_str!("./md/attributes/hx-preserve.md"),
        ),
        ("prompt", include_str!("./md/attributes/hx-prompt.md")),
        (
            "hx-replace-url",
            include_str!("./md/attributes/hx-replace-url.md"),
        ),
        ("request", include_str!("./md/attributes/hx-request.md")),
        ("sync", include_str!("./md/attributes/hx-sync.md")),
        (
            "hx-validate",
            include_str!("./md/attributes/hx-validate.md"),
        ),
    ];

    to_hx_completion(values)
}

impl From<&(&str, &str)> for HxCompletion {
    fn from((name, desc): &(&str, &str)) -> Self {
        Self {
            name: name.to_string(),
            desc: desc.to_string(),
        }
    }
}

pub fn create_basic_values() -> HashMap<String, Vec<HxCompletion>> {
    let mut hm = HashMap::new();

    let hx_swap = to_hx_completion(vec![
        ("innerHTML", include_str!("./md/hx-swap/innerHTML.md")),
        ("outerHTML", include_str!("./md/hx-swap/outerHTML.md")),
        ("afterbegin", include_str!("./md/hx-swap/afterbegin.md")),
        ("afterend", include_str!("./md/hx-swap/afterend.md")),
        ("beforebegin", include_str!("./md/hx-swap/beforebegin.md")),
        ("beforeend", include_str!("./md/hx-swap/beforeend.md")),
        ("delete", include_str!("./md/hx-swap/delete.md")),
        ("none", include_str!("./md/hx-swap/none.md")),
    ]);
    hm.insert(String::from("hx-swap"), hx_swap);

    let hx_target = to_hx_completion(vec![
        ("closest", include_str!("./md/hx-target/closest.md")),
        ("find", include_str!("./md/hx-target/find.md")),
        ("next", include_str!("./md/hx-target/next.md")),
        ("prev", include_str!("./md/hx-target/prev.md")),
        ("this", include_str!("./md/hx-target/this.md")),
    ]);
    hm.insert(String::from("hx-target"), hx_target);

    let hx_boost = to_hx_completion(vec![
        ("true", include_str!("./md/hx-boost/true.md")),
        ("false", include_str!("./md/hx-boost/false.md")),
    ]);
    hm.insert(String::from("hx-boost"), hx_boost);

    let hx_trigger = to_hx_completion(vec![
        ("click", include_str!("./md/hx-trigger/click.md")),
        ("once", include_str!("./md/hx-trigger/once.md")),
        ("changed", include_str!("./md/hx-trigger/changed.md")),
        ("delay:", include_str!("./md/hx-trigger/delay.md")),
        ("throttle:", include_str!("./md/hx-trigger/throttle.md")),
        ("from:", include_str!("./md/hx-trigger/from.md")),
        ("target:", include_str!("./md/hx-trigger/target.md")),
        ("consume", include_str!("./md/hx-trigger/consume.md")),
        ("queue:", include_str!("./md/hx-trigger/queue.md")),
        ("keyup", include_str!("./md/hx-trigger/keyup.md")),
        ("load", include_str!("./md/hx-trigger/load.md")),
        ("revealed", include_str!("./md/hx-trigger/revealed.md")),
        ("intersect", include_str!("./md/hx-trigger/intersect.md")),
        ("every", include_str!("./md/hx-trigger/every.md")),
    ]);
    hm.insert(String::from("hx-trigger"), hx_trigger);

    let hx_ext = to_hx_completion(vec![
        ("ajax-header", include_str!("./md/hx-ext/ajax-header.md")),
        ("alpine-morph", include_str!("./md/hx-ext/alpine-morph.md")),
        ("class-tools", include_str!("./md/hx-ext/class-tools.md")),
        (
            "client-side-templates",
            include_str!("./md/hx-ext/client-side-templates.md"),
        ),
        ("debug", include_str!("./md/hx-ext/debug.md")),
        (
            "disable-element",
            include_str!("./md/hx-ext/disable-element.md"),
        ),
        ("event-header", include_str!("./md/hx-ext/event-header.md")),
        ("head-support", include_str!("./md/hx-ext/head-support.md")),
        ("include-vals", include_str!("./md/hx-ext/include-vals.md")),
        ("json-enc", include_str!("./md/hx-ext/json-enc.md")),
        ("morph", include_str!("./md/hx-ext/morph.md")),
        (
            "loading-states",
            include_str!("./md/hx-ext/loading-states.md"),
        ),
        (
            "method-override",
            include_str!("./md/hx-ext/method-override.md"),
        ),
        (
            "morphdom-swap",
            include_str!("./md/hx-ext/morphdom-swap.md"),
        ),
        ("multi-swap", include_str!("./md/hx-ext/multi-swap.md")),
        ("path-deps", include_str!("./md/hx-ext/path-deps.md")),
        ("preload", include_str!("./md/hx-ext/preload.md")),
        ("remove-me", include_str!("./md/hx-ext/remove-me.md")),
        (
            "response-targets",
            include_str!("./md/hx-ext/response-targets.md"),
        ),
        ("restored", include_str!("./md/hx-ext/restored.md")),
        ("sse", include_str!("./md/hx-ext/sse.md")),
        ("ws", include_str!("./md/hx-ext/ws.md")),
    ]);
    hm.insert(String::from("hx-ext"), hx_ext);

    let hx_push_ul = to_hx_completion(vec![
        ("true", include_str!("./md/hx-push-url/true.md")),
        ("false", include_str!("./md/hx-push-url/false.md")),
    ]);
    hm.insert(String::from("hx-push-ul"), hx_push_ul);

    let hx_swap_ob = to_hx_completion(vec![
        ("true", include_str!("./md/hx-swap-oob/true.md")),
        ("innerHTML", include_str!("./md/hx-swap/innerHTML.md")),
        ("outerHTML", include_str!("./md/hx-swap/outerHTML.md")),
        ("afterbegin", include_str!("./md/hx-swap/afterbegin.md")),
        ("afterend", include_str!("./md/hx-swap/afterend.md")),
        ("beforebegin", include_str!("./md/hx-swap/beforebegin.md")),
        ("beforeend", include_str!("./md/hx-swap/beforeend.md")),
        ("delete", include_str!("./md/hx-swap/delete.md")),
        ("none", include_str!("./md/hx-swap/none.md")),
    ]);
    hm.insert(String::from("hx-swap-ob"), hx_swap_ob);

    let hx_history = to_hx_completion(vec![("false", include_str!("./md/hx-history/false.md"))]);
    hm.insert(String::from("hx-history"), hx_history);

    let hx_params = to_hx_completion(vec![
        ("*", include_str!("./md/hx-params/star.md")),
        ("none", include_str!("./md/hx-params/none.md")),
        ("not", include_str!("./md/hx-params/not.md")),
    ]);
    hm.insert(String::from("hx-params"), hx_params);

    let hx_replace_ul = to_hx_completion(vec![
        ("true", include_str!("./md/hx-replace-url/true.md")),
        ("false", include_str!("./md/hx-replace-url/false.md")),
    ]);
    hm.insert(String::from("hx-replace-ul"), hx_replace_ul);

    let hx_sync = to_hx_completion(vec![
        ("drop", include_str!("./md/hx-sync/drop.md")),
        ("abort", include_str!("./md/hx-sync/abort.md")),
        ("replace", include_str!("./md/hx-sync/replace.md")),
        ("queue", include_str!("./md/hx-sync/queue.md")),
    ]);
    hm.insert(String::from("hx-sync"), hx_sync);

    hm
}

fn to_hx_completion(values: Vec<(&str, &str)>) -> Vec<HxCompletion> {
    values.iter().filter_map(|x| x.try_into().ok()).collect()
}
