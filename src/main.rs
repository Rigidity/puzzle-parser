#![allow(dead_code)]

use chia_wallet::{cat::*, did::*, nft::*, singleton::*, standard::*};
use clvm_traits::{FromClvmError, FromNodePtr};
use clvm_utils::{tree_hash, CurriedProgram};
use clvmr::{serde::node_from_bytes, Allocator, NodePtr};

type Singleton<T> = CurriedProgram<NodePtr, SingletonArgs<T>>;
type NftState<T> = CurriedProgram<NodePtr, NftStateLayerArgs<T, NftMetadata>>;
type NftOwnership<T> = CurriedProgram<NodePtr, NftOwnershipLayerArgs<TransferProgram, T>>;
type TransferProgram = CurriedProgram<NodePtr, NftRoyaltyTransferPuzzleArgs>;
type Did<T> = CurriedProgram<NodePtr, DidArgs<T, NodePtr>>;
type Cat<T> = CurriedProgram<NodePtr, CatArgs<T>>;
type Standard = CurriedProgram<NodePtr, StandardArgs>;

#[derive(Debug, Clone)]
struct Spend<P, S> {
    puzzle: P,
    solution: S,
}

type StandardSpend = Spend<Standard, StandardSolution<NodePtr, NodePtr>>;
type CatSpend = Spend<Cat<NodePtr>, CatSolution<NodePtr>>;
type NftSpend = Spend<
    Singleton<NftState<NftOwnership<NodePtr>>>,
    SingletonSolution<NftStateLayerSolution<NftOwnershipLayerSolution<NodePtr>>>,
>;
type DidSpend = Spend<Singleton<Did<NodePtr>>, SingletonSolution<DidSolution<NodePtr>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CatVersion {
    Cat1,
    Cat2,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum KnownSpend {
    Standard(StandardSpend),
    Cat {
        spend: CatSpend,
        version: CatVersion,
    },
    Nft(NftSpend),
    Did(DidSpend),
}

impl KnownSpend {
    pub fn parse(
        a: &Allocator,
        puzzle_ptr: NodePtr,
        solution_ptr: NodePtr,
    ) -> Result<Self, FromClvmError> {
        let uncurried = CurriedProgram::<NodePtr, NodePtr>::from_node_ptr(a, puzzle_ptr)?;
        let mod_hash = tree_hash(a, uncurried.program);

        let result = if mod_hash == STANDARD_PUZZLE_HASH {
            let puzzle = FromNodePtr::from_node_ptr(a, puzzle_ptr)?;
            let solution = FromNodePtr::from_node_ptr(a, solution_ptr)?;

            Self::Standard(StandardSpend { puzzle, solution })
        } else if mod_hash == CAT_PUZZLE_HASH || mod_hash == CAT_PUZZLE_HASH_V1 {
            let version = if mod_hash == CAT_PUZZLE_HASH {
                CatVersion::Cat2
            } else {
                CatVersion::Cat1
            };

            let puzzle = FromNodePtr::from_node_ptr(a, puzzle_ptr)?;
            let solution = FromNodePtr::from_node_ptr(a, solution_ptr)?;

            let spend = CatSpend { puzzle, solution };
            Self::Cat { spend, version }
        } else if mod_hash == SINGLETON_TOP_LAYER_PUZZLE_HASH {
            let singleton =
                Singleton::<CurriedProgram<NodePtr, NodePtr>>::from_node_ptr(a, puzzle_ptr)?;
            let uncurried = singleton.args.inner_puzzle;
            let mod_hash = tree_hash(a, uncurried.program);

            if mod_hash == DID_INNER_PUZZLE_HASH {
                let puzzle = FromNodePtr::from_node_ptr(a, puzzle_ptr)?;
                let solution = FromNodePtr::from_node_ptr(a, solution_ptr)?;

                Self::Did(DidSpend { puzzle, solution })
            } else if mod_hash == NFT_STATE_LAYER_PUZZLE_HASH {
                let puzzle =
                    Singleton::<NftState<NftOwnership<NodePtr>>>::from_node_ptr(a, puzzle_ptr)?;
                let solution = FromNodePtr::from_node_ptr(a, solution_ptr)?;

                let ownership = puzzle.args.inner_puzzle.args.inner_puzzle.clone();
                let ownership_mod_hash = tree_hash(a, ownership.program);

                if ownership_mod_hash != NFT_OWNERSHIP_LAYER_PUZZLE_HASH {
                    return Err(FromClvmError::Custom(
                        "unknown nft ownership layer".to_string(),
                    ));
                }

                Self::Nft(NftSpend { puzzle, solution })
            } else {
                return Err(FromClvmError::Custom("unknown singleton".to_string()));
            }
        } else {
            return Err(FromClvmError::Custom("unknown puzzle".to_string()));
        };

        Ok(result)
    }
}

fn main() {
    let puzzle = hex::decode(include_str!("../puzzle.txt").trim()).unwrap();
    let solution = hex::decode(include_str!("../solution.txt").trim()).unwrap();

    let mut a = Allocator::new();
    let puzzle_ptr = node_from_bytes(&mut a, &puzzle).unwrap();
    let solution_ptr = node_from_bytes(&mut a, &solution).unwrap();
    let spend = KnownSpend::parse(&a, puzzle_ptr, solution_ptr).unwrap();

    dbg!(spend);
}
