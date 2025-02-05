use crate::{data_io::MAX_DATA_BRANCH_LEN, BlockHashNum, SessionBoundaries};
use codec::{Decode, Encode};
use sp_runtime::{
    traits::{Block as BlockT, NumberFor},
    SaturatedConversion,
};
use std::{
    hash::{Hash, Hasher},
    ops::Index,
};

/// Represents a proposal we obtain from another node. Note that since the proposal might come from
/// a malicious node there is no guarantee that the block hashes in the proposal correspond to real blocks
/// and even if they do then they could not match the provided number. Moreover, the block number in the
/// proposal might be completely arbitrary and hence we perform initial validation of the block number and
/// the branch length before we transform it into a safer `AlephProposal` type that guarantees we will not
/// fail on  any integer over- or underflows.
/// We expect that honest nodes create UnvalidatedAlephProposal {branch: [h_0, h_1, ..., h_n], number: num} objects
/// that represent an ascending sequence of blocks b_0, b_1, ..., b_n satisfying the following conditions:
///     1) hash(b_i) = h_i for i = 0, 1, ..., n,
///     2) parent(b_{i+1}) = b_i for i = 0, 1, ..., (n-1),
///     3) height(b_n) = num,
///     4) The parent of b_0 has been finalized (prior to creating this AlephData).
/// Such an UnvalidatedAlephProposal  object should be thought of as a proposal for block b_n to be finalized.
/// We refer for to `DataProvider` for a precise description of honest nodes' algorithm of creating proposals.
#[derive(Clone, Debug, Encode, Decode)]
pub struct UnvalidatedAlephProposal<B: BlockT> {
    pub branch: Vec<B::Hash>,
    pub number: NumberFor<B>,
}

// Need to be implemented manually, as deriving does not work (`BlockT` is not `Hash`).
impl<B: BlockT> Hash for UnvalidatedAlephProposal<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.branch.hash(state);
        self.number.hash(state);
    }
}

// Clippy does not allow deriving PartialEq when implementing Hash manually
impl<B: BlockT> PartialEq for UnvalidatedAlephProposal<B> {
    fn eq(&self, other: &Self) -> bool {
        self.number.eq(&other.number) && self.branch.eq(&other.branch)
    }
}

impl<B: BlockT> Eq for UnvalidatedAlephProposal<B> {}

impl<B: BlockT> UnvalidatedAlephProposal<B> {
    pub(crate) fn new(branch: Vec<B::Hash>, block_number: NumberFor<B>) -> Self {
        UnvalidatedAlephProposal {
            branch,
            number: block_number,
        }
    }

    pub(crate) fn validate_bounds(
        &self,
        session_boundaries: &SessionBoundaries<B>,
    ) -> Option<AlephProposal<B>> {
        if self.branch.len() > MAX_DATA_BRANCH_LEN as usize {
            return None;
        }
        if self.branch.is_empty() {
            return None;
        }
        if self.number < <NumberFor<B>>::saturated_from(self.branch.len()) {
            // Note that this also excludes branches starting at the genesis (0th) block.
            return None;
        }

        let bottom_block = self.number - <NumberFor<B>>::saturated_from(self.branch.len() - 1);
        let top_block = self.number;
        if session_boundaries.first_block() <= bottom_block
            && top_block <= session_boundaries.last_block()
        {
            return Some(AlephProposal {
                branch: self.branch.clone(),
                number: self.number,
            });
        }
        None
    }
}

/// A version of UnvalidatedAlephProposal that has been initially validated and fits
/// within session bounds.
#[derive(Clone, Debug, Encode, Decode)]
pub struct AlephProposal<B: BlockT> {
    branch: Vec<B::Hash>,
    number: NumberFor<B>,
}

// Need to be implemented manually, as deriving does not work (`BlockT` is not `Hash`).
impl<B: BlockT> Hash for AlephProposal<B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.branch.hash(state);
        self.number.hash(state);
    }
}

// Clippy does not allow deriving PartialEq when implementing Hash manually
impl<B: BlockT> PartialEq for AlephProposal<B> {
    fn eq(&self, other: &Self) -> bool {
        self.number.eq(&other.number) && self.branch.eq(&other.branch)
    }
}

impl<B: BlockT> Eq for AlephProposal<B> {}

impl<B: BlockT> Index<usize> for AlephProposal<B> {
    type Output = B::Hash;
    fn index(&self, index: usize) -> &Self::Output {
        &self.branch[index]
    }
}

impl<B: BlockT> AlephProposal<B> {
    /// Outputs the length the branch.
    pub fn len(&self) -> usize {
        self.branch.len()
    }

    /// Outputs the highest block in the branch.
    pub fn top_block(&self) -> BlockHashNum<B> {
        (
            *self
                .branch
                .last()
                .expect("cannot be empty for correct data"),
            self.number_top_block(),
        )
            .into()
    }

    /// Outputs the lowest block in the branch.
    pub fn bottom_block(&self) -> BlockHashNum<B> {
        // Assumes that the data is within bounds
        (
            *self
                .branch
                .first()
                .expect("cannot be empty for correct data"),
            self.number_bottom_block(),
        )
            .into()
    }

    /// Outputs the number one below the lowest block in the branch.
    pub fn number_below_branch(&self) -> NumberFor<B> {
        // Assumes that data is within bounds
        self.number - <NumberFor<B>>::saturated_from(self.branch.len())
    }

    /// Outputs the number of the lowest block in the branch.
    pub fn number_bottom_block(&self) -> NumberFor<B> {
        // Assumes that data is within bounds
        self.number - <NumberFor<B>>::saturated_from(self.branch.len() - 1)
    }

    /// Outputs the number of the highest block in the branch.
    pub fn number_top_block(&self) -> NumberFor<B> {
        self.number
    }

    /// Outputs the block corresponding to the number in the proposed branch in case num is
    /// between the lowest and highest block number of the branch. Otherwise returns None.
    pub fn block_at_num(&self, num: NumberFor<B>) -> Option<BlockHashNum<B>> {
        if self.number_bottom_block() <= num && num <= self.number_top_block() {
            let ind: usize = (num - self.number_bottom_block()).saturated_into();
            return Some((self.branch[ind], num).into());
        }
        None
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum PendingProposalStatus {
    PendingTopBlock,
    TopBlockImportedButIncorrectBranch,
    TopBlockImportedButNotFinalizedAncestor,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ProposalStatus<B: BlockT> {
    Finalize(BlockHashNum<B>),
    Ignore,
    Pending(PendingProposalStatus),
}

#[cfg(test)]
mod tests {
    use super::UnvalidatedAlephProposal;
    use crate::{data_io::MAX_DATA_BRANCH_LEN, SessionBoundaries, SessionId, SessionPeriod};
    use sp_core::hash::H256;
    use substrate_test_runtime_client::runtime::Block;

    #[test]
    fn too_long_proposal_is_invalid() {
        let session_boundaries = SessionBoundaries::<Block>::new(SessionId(1), SessionPeriod(20));
        let session_end = session_boundaries.last_block();
        let branch = vec![H256::default(); MAX_DATA_BRANCH_LEN + 1];
        let proposal = UnvalidatedAlephProposal::new(branch, session_end);
        assert_eq!(proposal.validate_bounds(&session_boundaries), None);
    }

    #[test]
    fn proposal_not_within_session_is_invalid() {
        let session_boundaries = SessionBoundaries::<Block>::new(SessionId(1), SessionPeriod(20));
        let session_start = session_boundaries.first_block();
        let session_end = session_boundaries.last_block();
        let branch = vec![H256::default(); 2];

        let proposal = UnvalidatedAlephProposal::new(branch.clone(), session_start);
        assert_eq!(proposal.validate_bounds(&session_boundaries), None);

        let proposal = UnvalidatedAlephProposal::new(branch, session_end + 1);
        assert_eq!(proposal.validate_bounds(&session_boundaries), None);
    }

    #[test]
    fn proposal_starting_at_zero_block_is_invalid() {
        let session_boundaries = SessionBoundaries::<Block>::new(SessionId(0), SessionPeriod(20));
        let branch = vec![H256::default(); 2];

        let proposal = UnvalidatedAlephProposal::new(branch, 1);
        assert_eq!(proposal.validate_bounds(&session_boundaries), None);
    }

    #[test]
    fn valid_proposal_is_validated_positively() {
        let session_boundaries = SessionBoundaries::<Block>::new(SessionId(0), SessionPeriod(20));

        let branch = vec![H256::default(); MAX_DATA_BRANCH_LEN];
        let proposal = UnvalidatedAlephProposal::new(branch, (MAX_DATA_BRANCH_LEN + 1) as u64);
        assert!(proposal.validate_bounds(&session_boundaries).is_some());

        let branch = vec![H256::default(); 1];
        let proposal = UnvalidatedAlephProposal::new(branch, (MAX_DATA_BRANCH_LEN + 1) as u64);
        assert!(proposal.validate_bounds(&session_boundaries).is_some());
    }
}
