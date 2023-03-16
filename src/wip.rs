//! Generic Work-in-progress support for VHL graphs.
use std::collections::HashMap;
use std::hash::Hash;
use nid::NID;
use vid::VID;
use {vhl, vhl::HiLoPart};
use swarm::QID;

pub type WIPHashMap<K,V> = HashMap<K,V>;

/// Work in progress for Swarms.
#[derive(PartialEq,Debug,Copy,Clone)]
pub enum WIP { Fresh, Done(NID), Parts(vhl::VHLParts) }



/// Response message. TWIP is whatever type is used for WIP hi/lo nodes.
#[derive(PartialEq,Debug)]
pub enum RMsg<TWIP> {
  /// resolved to a nid
  Nid(NID),
  /// a simple node needs to be constructed:
  Vhl{v:VID, hi:NID, lo:NID, invert:bool},
  /// other work in progress
  Wip{v:VID, hi:TWIP, lo:TWIP, invert:bool},
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID),
  /// return stats about the memo cache
  MemoStats { tests: u64, fails: u64 }}

impl<TWIP> std::ops::Not for RMsg<TWIP> {
    type Output = RMsg<TWIP>;
    fn not(self)->RMsg<TWIP> {
      match self {
        RMsg::Nid(n) => RMsg::Nid(!n),
        RMsg::Vhl{v,hi,lo,invert} => RMsg::Vhl{v,hi,lo,invert:!invert},
        RMsg::Wip{v,hi,lo,invert} => RMsg::Wip{v,hi,lo,invert:!invert},
        RMsg::Ret(n) => RMsg::Ret(!n),
        RMsg::MemoStats{ tests:_, fails: _} => panic!("not(MemoStats)? This makes no sense.") }}}


/// Helps track dependencies between WIP tasks
#[derive(Debug,Copy,Clone)]
pub struct Dep { pub qid: QID, pub part: HiLoPart, pub invert: bool }
impl Dep{
  pub fn new(qid: QID, part: HiLoPart, invert: bool)->Dep { Dep{qid, part, invert} }}


// TODO: come up with a better name for this.
#[derive(Debug, Default)]
pub struct WorkState<Q:Eq+Hash+Default> {
  /// stores work in progress during a run:
  pub wip: WIPHashMap<QID,WIP>,
  /// stores dependencies during a run. The bool specifies whether to invert.
  pub deps: WIPHashMap<QID, Vec<Dep>>,
  /// track ongoing tasks so we don't duplicate work in progress:
  pub qid: WIPHashMap<Q,QID>,
  /// track new queries so they can eventually be cached
  // !! not sure this one belongs here, but we'll see.
  pub qs: WIPHashMap<QID,Q>}

impl<Q:Eq+Hash+Default> WorkState<Q> {
  pub fn new() -> Self { Self::default() }
  pub fn resolve_part(&mut self, qid:&QID, part:HiLoPart, nid:NID, invert: bool) {
    if let Some(WIP::Parts(ref mut parts)) = self.wip.get_mut(qid) {
      let n = if invert { !nid } else { nid };
      trace!("   !! set {:?} for q{:?} to {}", part, qid, n);
      if part == HiLoPart::HiPart { parts.hi = Some(n) } else { parts.lo = Some(n) }}
    else { warn!("???? got a part for {:?} that was already done!", qid) }}}
