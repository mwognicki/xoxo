use crate::bus::{Command};
use crate::chat::structs::{ChatPath};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub type HandleFuture<'a, T> = BoxFuture<'a, T>;

pub trait AgentHandle: Send + Sync {
    fn chat_id(&self) -> &Uuid;
    fn path(&self) -> &ChatPath;
    fn send(&self, cmd: Command) -> HandleFuture<'_, Result<(), HandleError>>;
    fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HandleError {
    #[error("handle not found for chat {chat_id}")]
    NotFound { chat_id: Uuid },
    #[error("user messages may only be sent to root agents")]
    NonRootUserMessage,
    #[error("agent handle is closed")]
    Closed,
}

#[derive(Default)]
pub struct HandleRegistry {
    handles: HashMap<Uuid, Arc<dyn AgentHandle>>,
}

impl HandleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, handle: Arc<dyn AgentHandle>) -> Option<Arc<dyn AgentHandle>> {
        self.handles.insert(*handle.chat_id(), handle)
    }

    pub fn get(&self, chat_id: &Uuid) -> Option<Arc<dyn AgentHandle>> {
        self.handles.get(chat_id).cloned()
    }

    pub fn remove(&mut self, chat_id: &Uuid) -> Option<Arc<dyn AgentHandle>> {
        self.handles.remove(chat_id)
    }

    pub fn roots(&self) -> Vec<Arc<dyn AgentHandle>> {
        self.handles
            .values()
            .filter(|handle| handle.path().depth() == 0)
            .cloned()
            .collect()
    }

    pub fn children_of(&self, parent: &Uuid) -> Vec<Arc<dyn AgentHandle>> {
        self.handles
            .values()
            .filter(|handle| handle.path().parent_id() == Some(parent))
            .cloned()
            .collect()
    }

    pub fn subtree(&self, root: &Uuid) -> Vec<Arc<dyn AgentHandle>> {
        self.handles
            .values()
            .filter(|handle| handle.path().root_id() == root)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt;
    use crate::bus::UserMessage;
    use crate::chat::structs::ChatTextRole;
    use super::*;

    struct StubHandle {
        chat_id: Uuid,
        path: ChatPath,
    }

    impl StubHandle {
        fn new(path: ChatPath) -> Self {
            let chat_id = *path.current();
            Self { chat_id, path }
        }
    }

    impl AgentHandle for StubHandle {
        fn chat_id(&self) -> &Uuid {
            &self.chat_id
        }

        fn path(&self) -> &ChatPath {
            &self.path
        }

        fn send(&self, cmd: Command) -> HandleFuture<'_, Result<(), HandleError>> {
            async move {
                match cmd {
                    Command::SubmitUserMessage { .. } => Ok(()),
                    Command::SendUserMessage { .. } if self.path.depth() > 0 => {
                        Err(HandleError::NonRootUserMessage)
                    }
                    Command::SendUserMessage { message, .. } => {
                        let _role = message.role;
                        let _content = message.content;
                        Ok(())
                    }
                    Command::Shutdown { .. } => Ok(()),
                }
            }
            .boxed()
        }

        fn shutdown(&self) -> HandleFuture<'_, Result<(), HandleError>> {
            async { Ok(()) }.boxed()
        }
    }

    fn sample_user_message(path: ChatPath) -> Command {
        Command::SendUserMessage {
            path,
            message: UserMessage {
                role: ChatTextRole::User,
                content: "hello".to_string(),
            },
        }
    }

    fn sorted_ids(handles: Vec<Arc<dyn AgentHandle>>) -> Vec<Uuid> {
        let mut ids = handles.into_iter().map(|handle| *handle.chat_id()).collect::<Vec<_>>();
        ids.sort();
        ids
    }

    #[test]
    fn roots_returns_only_root_handles() {
        let root = Uuid::from_u128(1);
        let child = Uuid::from_u128(2);
        let mut registry = HandleRegistry::new();

        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child]))));

        assert_eq!(sorted_ids(registry.roots()), vec![root]);
    }

    #[test]
    fn children_of_returns_only_direct_children() {
        let root = Uuid::from_u128(1);
        let child_a = Uuid::from_u128(2);
        let child_b = Uuid::from_u128(3);
        let grandchild = Uuid::from_u128(4);
        let other_root = Uuid::from_u128(5);
        let mut registry = HandleRegistry::new();

        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child_a]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child_b]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child_a, grandchild]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![other_root]))));

        assert_eq!(sorted_ids(registry.children_of(&root)), vec![child_a, child_b]);
        assert_eq!(sorted_ids(registry.children_of(&child_a)), vec![grandchild]);
    }

    #[test]
    fn subtree_returns_every_handle_in_the_same_tree() {
        let root = Uuid::from_u128(1);
        let child = Uuid::from_u128(2);
        let grandchild = Uuid::from_u128(3);
        let other_root = Uuid::from_u128(4);
        let other_child = Uuid::from_u128(5);
        let mut registry = HandleRegistry::new();

        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![root, child, grandchild]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![other_root]))));
        registry.insert(Arc::new(StubHandle::new(ChatPath(vec![other_root, other_child]))));

        assert_eq!(sorted_ids(registry.subtree(&root)), vec![root, child, grandchild]);
        assert_eq!(sorted_ids(registry.subtree(&other_root)), vec![other_root, other_child]);
    }

    #[test]
    fn non_root_user_message_is_rejected() {
        let child = StubHandle::new(ChatPath(vec![Uuid::from_u128(1), Uuid::from_u128(2)]));
        let result = futures::executor::block_on(child.send(sample_user_message(child.path.clone())));
        assert_eq!(result, Err(HandleError::NonRootUserMessage));
    }
}
