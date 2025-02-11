//! wrap bex as a python module
extern crate bex as bex_rs;
extern crate fxhash;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::exceptions::PyException;
use bex_rs::{Base, GraphViz, ast::ASTBase, BddBase, nid::{I,O,NID}, vid::VID, cur::Cursor, Reg};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[pyclass(name="NID")] struct PyNID(NID);
#[pyclass(name="VID")] struct PyVID(VID);
#[pyclass(name="ASTBase")] struct PyASTBase(ASTBase);
#[pyclass(name="BddBase")] struct PyBddBase(Arc<Mutex<BddBase>>);
#[pyclass(name="Reg")] struct PyReg(Reg);
#[pyclass(name="Cursor")] struct PyCursor(Option<Cursor>);

enum BexErr { NegVar, NegVir }
impl std::convert::From<BexErr> for PyErr {
  fn from(err: BexErr) -> PyErr {
    match err {
      BexErr::NegVar => PyException::new_err("var(i) expects i >= 0"),
      BexErr::NegVir => PyException::new_err("vir(i) expects i >= 0") }}}

#[pymethods]
impl PyNID {
  fn is_const(&self)->bool { self.0.is_const() }
  fn is_lit(&self)->bool { self.0.is_lit() }
  fn is_vid(&self)->bool { self.0.is_vid() }
  fn _vid(&self)->PyVID { PyVID(self.0.vid()) }
  fn __eq__(&self, other:&PyNID)->bool { self.0 == other.0 }
  fn __invert__(&self)->PyNID { PyNID(!self.0) }
  fn __str__(&self) -> String { self.0.to_string() }
  fn __hash__(&self) -> u64 { fxhash::hash64(&self.0) }
  fn __repr__(&self) -> String { format!("<NID({:?})>", self.0) }}

#[pymethods]
impl PyVID {
  #[getter] fn ix(&self)->usize { self.0.vid_ix() }
  fn to_nid(&self)->PyNID { PyNID(NID::from_vid(self.0)) }
  fn __eq__(&self, other:&PyVID)->bool { self.0 == other.0 }
  fn __hash__(&self) -> u64 { fxhash::hash64(&self.0) }
  fn __str__(&self) -> String { self.0.to_string() }
  fn __repr__(&self) -> String { format!("<VID({:?})>", self.0) }}

#[pymethods]
impl PyASTBase {
  #[new] fn __new__()->Self { Self(ASTBase::empty()) }
  fn op_and(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID(self.0.and(x.0, y.0)) }
  fn op_xor(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID(self.0.xor(x.0, y.0)) }
  fn op_or(&mut self, x:&PyNID, y:&PyNID)->PyNID  { PyNID(self.0.or(x.0, y.0)) }
  fn to_dot(&self, x:&PyNID)->String { let mut s = String::new(); self.0.write_dot(x.0, &mut s); s }}

#[pymethods]
impl PyBddBase {
  #[new] fn __new__()->Self { Self(Arc::new(Mutex::new(BddBase::new()))) }
  fn op_and(&mut self, x:&PyNID, y:&PyNID)->PyNID { let mut base = self.0.lock().unwrap(); PyNID(base.and(x.0, y.0)) }
  fn op_xor(&mut self, x:&PyNID, y:&PyNID)->PyNID { let mut base = self.0.lock().unwrap(); PyNID(base.xor(x.0, y.0)) }
  fn op_or(&mut self, x:&PyNID, y:&PyNID)->PyNID  { let mut base = self.0.lock().unwrap(); PyNID(base.or(x.0, y.0)) }
  fn ite(&mut self, i:&PyNID, t:&PyNID, e:&PyNID)->PyNID {
    let mut base = self.0.lock().unwrap();
    PyNID(base.ite(i.0, t.0, e.0)) }
  fn eval(&mut self, x: &PyNID, kv: &Bound<'_, PyDict>) -> PyResult<PyNID> {
    let mut base = self.0.lock().unwrap();
    let mut rust_kv = HashMap::new();
    for (key, value) in kv.iter() {
      let py_vid: PyRef<PyVID> = key.extract().map_err(|_| PyException::new_err("Expected PyVID as key"))?;
      let py_nid: PyRef<PyNID> = value.extract().map_err(|_| PyException::new_err("Expected PyNID as value"))?;
      rust_kv.insert(py_vid.0, py_nid.0); }
    Ok(PyNID(base.eval(x.0, &rust_kv))) }
  fn __len__(&self)->usize { self.0.lock().unwrap().len() }
  fn get_vhl(&self, n:&PyNID)->(PyVID, PyNID, PyNID) {
    let base = self.0.lock().unwrap();
    let (v, hi, lo) = base.get_vhl(n.0); (PyVID(v), PyNID(hi), PyNID(lo))}
  fn to_dot(&self, x:&PyNID)->String {
    let base = self.0.lock().unwrap();
    let mut s = String::new(); base.write_dot(x.0, &mut s);  s }
  fn solution_count(&mut self, x: &PyNID) -> u64 {
    let mut base = self.0.lock().unwrap();
    base.solution_count(x.0) }
  fn first_solution(&self, n: &PyNID, nvars: usize) -> PyCursor {
    let base = self.0.lock().unwrap();
    PyCursor(base.first_solution(n.0, nvars)) }}

#[pymethods]
impl PyReg {
  #[getter]
  fn len(&self) -> usize { self.0.len() }
  fn as_usize(&self) -> usize { self.0.as_usize() }
  fn as_usize_rev(&self) -> usize { self.0.as_usize_rev() }
  fn hi_bits(&self) -> Vec<usize> { self.0.hi_bits() }}

#[pymethods]
impl PyCursor {
  #[getter] fn scope(&self) -> Option<PyReg> { self.0.as_ref().map(|c| PyReg(c.scope.clone())) }
  #[getter] fn at_end(&self) -> bool { self.0.is_none() }
  fn _advance(&mut self, base:&PyBddBase) {
    let base = base.0.lock().unwrap();
    if self.0.is_some() {
      let cur = self.0.take().unwrap();
      self.0 = base.next_solution(cur) }}}


#[pyfunction]
fn var(i:i32)->PyResult<PyVID> { if i<0 { Err(BexErr::NegVar.into()) } else { Ok(PyVID(VID::var(i as u32))) }}
#[pyfunction]
fn vir(i:i32)->PyResult<PyVID> { if i<0 { Err(BexErr::NegVir.into()) } else { Ok(PyVID(VID::vir(i as u32))) }}
#[pyfunction]
fn nvar(i:i32)->PyResult<PyNID> { var(i).map(|v| v.to_nid()) }
#[pyfunction]
fn nvir(i:i32)->PyResult<PyNID> { vir(i).map(|v| v.to_nid()) }

#[pymodule]
fn bex(m: &Bound<'_, PyModule>)->PyResult<()> {
  m.add_class::<PyVID>()?;
  m.add_class::<PyNID>()?;
  m.add_class::<PyASTBase>()?;
  m.add_class::<PyBddBase>()?;
  m.add_class::<PyReg>()?;
  m.add_class::<PyCursor>()?;
  m.add("O", PyNID(O))?;
  m.add("I", PyNID(I))?;

  m.add_function(wrap_pyfunction!(var, m)?)?;
  m.add_function(wrap_pyfunction!(vir, m)?)?;
  m.add_function(wrap_pyfunction!(nvar, m)?)?;
  m.add_function(wrap_pyfunction!(nvir, m)?)?;

  Ok(())}
