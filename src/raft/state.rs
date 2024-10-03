#[derive(Debug, PartialEq)]
pub enum State {
    Follower,
    Candidate,
    Leader,
}
