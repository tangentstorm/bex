//! Generic Work-in-progress support for VHL graphs.
use std::default::Default;
use std::marker::PhantomData;
use std::{collections::HashMap};
use std::hash::Hash;
use nid::NID;
use vid::VID;
use vhl::{HiLoPart, VhlParts};
use bdd::{Norm, NormIteKey};
use dashmap::DashMap;



pub type WIPHashMap<K,V> = HashMap<K,V,fxhash::FxBuildHasher>;

#[derive(Debug,Copy,Clone)]
pub struct Dep<K> { pub dep: K, pub part: HiLoPart, pub invert: bool }
impl<K> Dep<K>{
  pub fn new(dep: K, part: HiLoPart, invert: bool)->Dep<K> { Dep{dep, part, invert} }}

#[derive(Debug, Default)]
pub struct Wip<K=NormIteKey, P=VhlParts> { pub parts : P, pub deps : Vec<Dep<K>> }

// TODO: wrap this with a smart pointer so Work::Done and Work::Todo are both usizes.
type WipRef<K=NormIteKey, P=VhlParts> = Wip<K, P>;

#[derive(Debug)]
pub enum Work<V, W=WipRef> { Todo(W), Done(V) }

impl<V,W> Default for Work<V, W> where W:Default {
    fn default() -> Self { Work::Todo(W::default()) }}

impl<V,W> Work<V,W> {

  pub fn is_todo(&self)->bool {
    if let Self::Todo(_) = self { true } else { false }}

  pub fn is_done(&self)->bool {
    if let Self::Done(_) = self { true } else { false }}

  pub fn unwrap(&self)->&V {
    if let Self::Done(v) = self { &v } else {
      panic!("cannot unwrap() a Work::Todo") }}

  pub fn wip_mut(&mut self)->&mut W {
    if let Self::Todo(w) = self { w } else {
      panic!("cannot get wip() from a Work::Done") }}

  pub fn wip(&self)->&W {
    if let Self::Todo(w) = self { &w } else {
      panic!("cannot get wip() from a Work::Done") }}}


#[derive(Debug, Default)]
pub struct WorkState<K=NormIteKey, V=NID, P=VhlParts> where K:Eq+Hash {
  _kvp: PhantomData<(K,V,P)>,
  pub cache: DashMap<K, Work<V, WipRef<K,P>>> }



// one step in the resolution of a query.
// !! to be replaced by direct calls to
//    work.cache.resolve_nid, resolve_vhl, resolve_part
#[derive(PartialEq,Debug)]
pub enum ResStep {
  /// resolved to a nid
  Nid(NID),
  /// a simple node needs to be constructed:
  Vhl{v:VID, hi:NID, lo:NID, invert:bool},
  /// other work in progress
  Wip{v:VID, hi:Norm, lo:Norm, invert:bool}}

impl std::ops::Not for ResStep {
  type Output = ResStep;
  fn not(self)->ResStep {
    match self {
      ResStep::Nid(n) => ResStep::Nid(!n),
      ResStep::Vhl{v,hi,lo,invert} => ResStep::Vhl{v,hi,lo,invert:!invert},
      ResStep::Wip{v,hi,lo,invert} => ResStep::Wip{v,hi,lo,invert:!invert} }}}

/// Response message.
#[derive(PartialEq,Debug)]
pub enum RMsg {
  Res(NormIteKey, ResStep),
  /// We've solved the whole problem, so exit the loop and return this nid.
  Ret(NID),
  /// return stats about the memo cache
  MemoStats { tests: u64, fails: u64 }}
