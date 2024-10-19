use crate::raft_protobufs::raft_client::RaftClient;
use crate::raft_protobufs::{
    AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse,
};
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tonic::{transport::Channel, Request, Response, Status};

use super::command::Command;
use super::log::LogEntry;
use super::state::State as NodeState;
#[derive(Debug, Deserialize)]
pub struct RemoteNode {
    pub node_uid: u64,
    pub address: String,
}

#[derive(Debug)]
pub struct LocalNode {
    data: HashMap<String, String>, // aka raft's state machine
    client_to_cluster: HashMap<u64, RaftClient<Channel>>,
    last_heartbeat: u64,
    election_timeout: u64,

    node_uid: u64,
    state: NodeState,

    // persistent state
    term: u64, /* latest term server has seen(initialized to 0 on first boot, increases
                * monotonically) */
    voted_for: Option<u64>, // candidateId that received vote in current term (or null if none)
    log: Vec<LogEntry>,     /* log entries; each entry contains command for state machine, and
                             * term when entry was received by
                             * leader (first index is 1) */

    // volatile state
    commit_index: u64, /* index of highest log entry known to be committed (initialized to 0,
                        * increases monotonically) */
    last_applied: u64, /* index of highest log entry applied to state machine (initialized to 0,
                        * increases monotonically) */

    // leader volatile state (Reinitialized after election)
    next_index: HashMap<u64, u64>, /* for each server, index of the next log entry to send to
                                    * that server
                                    * (initialized to leader last log index +
                                    * 1) */
    match_index: HashMap<u64, u64>, /* for each server, index of highest log entry known to be
                                     * replicated
                                     * on server (initialized to 0, increases
                                     * monotonically) */
}

fn current_time_millis() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis() as u64
}

// Rules for all servers:

// All servers
// 1. If commitIndex > lastApplied: increment lastApplied, apply
//    log[lastApplied] to state machine (§5.3)
// 2. If RPC request or response contains term T > currentTerm: set currentTerm
//    = T, convert to follower (§5.1)

// Followers (§5.2):
// 1. Respond to RPCs from candidates and leaders
// 2. If election timeout elapses without receiving AppendEntries RPC from
//    current leader or granting vote to candidate: convert to candidate

// Candidates (§5.2):
// 1. On conversion to candidate, start election:
/// #   a. Increment currentTerm
/// #   b. Vote for self
/// #   c. Reset election timer
/// #   d. Send RequestVote RPCs to all other servers
// 2. If votes received from majority of servers: become leader
// 3. If AppendEntries RPC received from new leader: convert to follower
// 4. If election timeout elapses: start new election

// Leaders (§5.2):
// 1. Upon election: send initial empty AppendEntries RPCs (heartbeat) to each server; repeat during
//    idle periods to prevent election timeouts (§5.2)
// 2. If command received from client: append entry to local log, respond after entry applied to
//    state machine (§5.3)
// 3. If last log index ≥ nextIndex for a follower: send AppendEntries RPC with log entries starting
//    at nextIndex:
/// #   a. If successful: update nextIndex and matchIndex for follower (§5.3)
/// #   b. If AppendEntries fails because of log inconsistency: decrement
//        nextIndex and retry (§5.3)
// 4. If there exists an N such that N > commitIndex, a majority of
//    matchIndex[i] ≥ N, and log[N].term == currentTerm: set commitIndex = N
//    (§5.3, §5.4)

impl LocalNode {
    pub fn new(id: u64, other_nodes: Vec<RemoteNode>) -> Self {
        LocalNode {
            data: HashMap::new(),
            client_to_cluster: HashMap::new(),
            last_heartbeat: current_time_millis(),
            election_timeout: Self::random_election_timeout(),
            node_uid: 0,
            state: NodeState::Follower,
            term: 0,
            voted_for: None,
            log: vec![],
            commit_index: 0,
            last_applied: 0,
            next_index: HashMap::new(),
            match_index: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: String) -> Status {
        self.data.insert(key, value);
        Status::ok("")
    }

    pub fn get(&mut self, key: String) -> Result<String, Status> {
        match self.data.get(&key) {
            Some(value) => Ok(value.clone()),
            None => Err(Status::not_found("".to_string())),
        }
    }

    pub fn del(&mut self, key: String) -> Status {
        self.data.remove(&key);
        Status::ok("")
    }

    // outer rpc callback holds Arc<Mutex<LocalNode>>, so rpc callback was
    // sequential and didn't need extra lock
    pub fn append_entries_impl(
        &mut self,
        request: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        // Receiver implementation:
        // 1. Reply false if term < currentTerm (§5.1)
        // 2. Reply false if log doesn’t contain an entry at prevLogIndex whose term
        //    matches prevLogTerm (§5.3)
        // 3. If an existing entry conflicts with a new one (same index but different
        //    terms), delete the existing entry and all that follow it (§5.3)
        // 4. Append any new entries not already in the log
        // 5. If leaderCommit > commitIndex, set commitIndex = min(leaderCommit, index
        //    of last new entry)
        let req = request.into_inner();

        // Step 1: check term
        if req.term < self.term {
            return Ok(Response::new(AppendEntriesResponse {
                term: self.term,
                success: false,
            }));
        }

        // Step 2: check log consistency
        if req.prev_log_index > 0 {
            if let Some(prev_log_entry) = self.log.get((req.prev_log_index - 1) as usize) {
                if prev_log_entry.term != req.prev_log_term {
                    return Ok(Response::new(AppendEntriesResponse {
                        term: self.term,
                        success: false,
                    }));
                }
            } else {
                return Ok(Response::new(AppendEntriesResponse {
                    term: self.term,
                    success: false,
                }));
            }
        }

        // Step 3: append new entries
        let mut index = req.prev_log_index as usize;
        for entry in req.entries {
            let entry2 = LogEntry::new(
                entry.term,
                Some(
                    Command::new(entry.command)
                        .map_err(|err| Status::internal(format!("{:?}", err)))?,
                ),
            );
            if index < self.log.len() {
                self.log[index] = entry2;
            } else {
                self.log.push(entry2);
            }
            index += 1;
        }

        // Step 4: update commit_index
        if req.leader_commit > self.commit_index {
            self.commit_index = std::cmp::min(req.leader_commit, self.log.len() as u64);
        }

        // this is hard to do standalone, it should be done with step 3
        // self.apply_log_entries();

        // Step 5: return response
        Ok(Response::new(AppendEntriesResponse {
            term: self.term,
            success: true,
        }))
    }

    // outer rpc callback holds Arc<Mutex<LocalNode>>, so rpc callback was
    // sequential and didn't need extra lock
    pub fn request_vote_impl(
        &mut self,
        request: Request<RequestVoteRequest>,
    ) -> Result<Response<RequestVoteResponse>, Status> {
        // Receiver implementation:
        // 1. Reply false if term < currentTerm (§5.1)
        // 2. If votedFor is null or candidateId, and candidate’s log is at least as
        //    up-to-date as receiver’s log, grant vote (§5.2, §5.4)
        let peer_term = request.get_ref().term;
        if peer_term >= self.term {
            self.term = peer_term;
            if request.get_ref().last_log_term > self.log.last().unwrap().term
                || (request.get_ref().last_log_term >= self.log.last().unwrap().term
                    && request.get_ref().last_log_index >= self.log.len() as u64)
            {
                self.voted_for = Some(request.get_ref().candidate_id);
                return Ok(Response::new(RequestVoteResponse {
                    term: self.term,
                    vote_granted: true,
                }));
            }
        }

        return Ok(Response::new(RequestVoteResponse {
            term: self.term,
            vote_granted: false,
        }));
    }

    pub async fn periodic_check_election_timeout(&mut self) {
        loop {
            sleep(tokio::time::Duration::from_millis(self.election_timeout)).await;
            if current_time_millis() - self.last_heartbeat >= self.election_timeout {
                self.start_election().await;
            }
        }
    }

    async fn start_election(&mut self) {
        self.term += 1;
        self.voted_for = Some(self.node_uid);
        let request = RequestVoteRequest {
            term: self.term,
            candidate_id: self.node_uid,
            last_log_index: self.log.len() as u64,
            last_log_term: if self.log.is_empty() {
                0
            } else {
                self.log.last().unwrap().term
            },
        };

        let responses = self.send_request_vote(request).await;

        let mut votes = 1;
        for response in responses {
            if response.vote_granted {
                votes += 1;
            }
        }

        let cluster_size = self.client_to_cluster.len() as u64;
        if votes > cluster_size / 2 {
            self.become_leader();
        } else {
            self.election_timeout = Self::random_election_timeout();
        }
    }

    async fn send_request_vote(&mut self, request: RequestVoteRequest) -> Vec<RequestVoteResponse> {
        let mut responses = vec![];
        for (_, client) in self.client_to_cluster.iter_mut() {
            let response = client
                .request_vote(request.clone())
                .await
                .unwrap()
                .into_inner();
            responses.push(response);
        }
        responses
    }

    async fn become_leader(&mut self) {}

    fn random_election_timeout() -> u64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(150..300)
    }
}
