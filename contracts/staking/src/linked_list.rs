use cosmwasm_std::{Addr, Storage, Uint128, StdResult};
use cosmwasm_storage::{
    bucket, bucket_read, singleton, singleton_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static NODE_KEY: &[u8] = b"node";
static LINKED_LIST_KEY: &[u8] = b"linked_list";

// node storage
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Node {
    pub receiver: Addr,
    pub value: Uint128,
    pub height: u64,
    pub prev: u64,
    pub next: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct NodeWithId {
    pub id: u64,
    pub info: Node
}

// linked-list storage
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LinkedList {
    pub head_id: u64,
    pub tail_id: u64,
    pub length: u64,
}

pub fn linked_list(storage: &mut dyn Storage) -> Singleton<LinkedList> {
    singleton(storage, LINKED_LIST_KEY)
}

pub fn linked_list_read(storage: &dyn Storage) -> ReadonlySingleton<LinkedList> {
    singleton_read(storage, LINKED_LIST_KEY)
}

pub fn node(storage: &mut dyn Storage) -> Bucket<Node> {
    bucket(storage, NODE_KEY)
}

pub fn node_read(storage: &dyn Storage) -> ReadonlyBucket<Node> {
    bucket_read(storage, NODE_KEY)
}

pub fn node_update_value(
    storage: &mut dyn Storage, 
    node_id: u64, 
    value: Uint128
) -> StdResult<()> {
    let node_key = &node_id.to_be_bytes();
    let mut cur_node = node(storage).load(node_key)?;
    cur_node.value = value;
    node(storage).save(node_key, &cur_node)?;
    
    Ok(())
}

pub fn linked_list_append(
    storage: &mut dyn Storage, 
    receiver: Addr, 
    value: Uint128, 
    height: u64
) -> StdResult<u64> {
    let mut state = linked_list(storage).load()?;
    let mut new_node_prev = 0;
    let new_node_id = state.tail_id + 1;
    if state.length == 0 {
        // empty LinkedList
        state.head_id = new_node_id;
    } else {
        // append to tail
        let tail_node_key = &state.tail_id.to_be_bytes();
        let mut tail_node = node(storage).load(tail_node_key)?;
        tail_node.next = new_node_id;
        node(storage).save(tail_node_key, &tail_node)?;
        new_node_prev = state.tail_id;
    }

    // create new node
    let new_node = Node {
        receiver: receiver,
        value: value,
        height: height,
        prev: new_node_prev,
        next: 0,
    };
    node(storage).save(&new_node_id.to_be_bytes(), &new_node)?;

    // update tail to new node
    state.tail_id = new_node_id;
    state.length += 1;
    // update linked list
    linked_list(storage).save(&state)?;

    Ok(new_node_id)
}

pub fn linked_list_clear(storage: &mut dyn Storage) -> StdResult<()> {
    let mut state = linked_list(storage).load()?;
    let mut cur_id = state.head_id;
    if cur_id == 0 {
        // empty list
        return Ok(());
    }

    let mut cur_node = node(storage).load(&cur_id.to_be_bytes())?;
    // iterate until tail
    while cur_id != state.tail_id {
        cur_id = cur_node.next;
        // we're done with this node
        node(storage).remove(&cur_id.to_be_bytes());
        // iterate to the next node
        cur_node = node(storage).load(&cur_id.to_be_bytes())?;
    }
    // delete the last node
    node(storage).remove(&cur_id.to_be_bytes());
    
    state.tail_id = 0;
    state.head_id = 0;
    state.length = 0;
    linked_list(storage).save(&state)?;

    Ok(())
}

pub fn linked_list_remove_head(storage: &mut dyn Storage) -> StdResult<()> {
    let state = linked_list(storage).load()?;
    if state.length == 1 {
        linked_list_clear(storage)?;
    } else {
        let mut state = linked_list(storage).load()?;
        let old_head_key = &state.head_id.to_be_bytes();
        let old_head = node(storage).load(old_head_key)?;
        let new_head_id = old_head.next;
        let new_head_key = &new_head_id.to_be_bytes();
        let mut new_head = node(storage).load(new_head_key)?;
        new_head.prev = 0;
        node(storage).remove(old_head_key);
        state.head_id = new_head_id;
        state.length -= 1;
        node(storage).save(new_head_key, &new_head)?;
        linked_list(storage).save(&state)?;
    }
    
    Ok(())
}

pub fn linked_list_remove_tail(storage: &mut dyn Storage) -> StdResult<()> {
    let state = linked_list(storage).load()?;
    if state.length == 1 {
        linked_list_clear(storage)?;
    } else {
        let mut state = linked_list(storage).load()?;
        let old_tail_key = &state.tail_id.to_be_bytes();
        let old_tail = node(storage).load(old_tail_key)?;
        let new_tail_id = old_tail.prev;
        let new_tail_key = &new_tail_id.to_be_bytes();
        let mut new_tail = node(storage).load(new_tail_key)?;
        new_tail.next = 0;
        node(storage).remove(old_tail_key);
        state.tail_id = new_tail_id;
        state.length -= 1;
        node(storage).save(new_tail_key, &new_tail)?;
        linked_list(storage).save(&state)?;
    }
    
    Ok(())
}

pub fn linked_list_remove(storage: &mut dyn Storage, node_id: u64) -> StdResult<()> {
    let mut state = linked_list(storage).load()?;
    if node_id == state.head_id {
        linked_list_remove_head(storage)?;
    } else if node_id == state.tail_id {
        linked_list_remove_tail(storage)?;
    } else {
        let cur_node_key = &node_id.to_be_bytes();
        let cur_node = node(storage).load(cur_node_key)?;
        let cur_next_node_key = &cur_node.next.to_be_bytes();
        let mut cur_next_node = node(storage).load(cur_next_node_key)?;
        let cur_prev_node_key = &cur_node.prev.to_be_bytes();
        let mut cur_prev_node = node(storage).load(cur_prev_node_key)?;
        cur_next_node.prev = cur_node.prev;
        cur_prev_node.next = cur_node.next;
        node(storage).remove(cur_node_key);
        state.length -= 1;
        node(storage).save(cur_prev_node_key, &cur_prev_node)?;
        node(storage).save(cur_next_node_key, &cur_next_node)?;
    }
    
    Ok(())
}

pub fn linked_list_get_list(storage: &dyn Storage, _count: u64) -> StdResult<Vec<NodeWithId>> {
    let mut queue_list: Vec<NodeWithId> = Vec::new();
    let state = linked_list_read(storage).load()?;
    if state.length > 0 {
        let count = if _count > state.length {state.length} else {_count};
        let mut node_id = state.head_id;
        let mut index = 0;
        while index < count {
            let cur_node_key = &node_id.to_be_bytes();
            let cur_node = node_read(storage).load(cur_node_key)?;
            let new_node_id = cur_node.next;
            queue_list.push(NodeWithId { id: node_id, info: cur_node });
            node_id = new_node_id;
            index += 1;
        }
    }
    
    return Ok(queue_list);
}

