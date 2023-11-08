//! wrap bex as a python module
extern crate bex;
use pyo3::prelude::*;
use pyo3::PyObjectProtocol;
use pyo3::exceptions::PyException;
use bex::{Base, GraphViz, ast::ASTBase, nid::{I,O,NID}, vid::VID};

#[pyclass(name="NID")] struct PyNID{ nid:NID }
#[pyclass(name="VID")] struct PyVID{ vid:VID }
#[pyclass(name="AST")] struct PyAST { base: ASTBase }

enum BexErr { NegVar, NegVir }
impl std::convert::From<BexErr> for PyErr {
  fn from(err: BexErr) -> PyErr {
    match err {
      BexErr::NegVar => PyException::new_err("var(i) expects i >= 0"),
      BexErr::NegVir => PyException::new_err("vir(i) expects i >= 0") }}}

#[pymethods]
impl PyNID {
  #[staticmethod]
  fn var(i:i32)->PyResult<Self> { if i<0 { Err(BexErr::NegVar.into()) } else { Ok(PyNID{ nid:NID::var(i as u32)}) }}
  #[staticmethod]
  fn vir(i:i32)->PyResult<Self> { if i<0 { Err(BexErr::NegVir.into()) } else { Ok(PyNID{ nid:NID::vir(i as u32)}) }}}

#[pyproto]
impl PyObjectProtocol for PyNID {
  fn __str__(&self) -> String { self.nid.to_string() }
  fn __repr__(&self) -> String { format!("<NID({:?})>", self.nid) }}

#[pyproto]
impl PyObjectProtocol for PyVID {
  fn __str__(&self) -> String { self.vid.to_string() }
  fn __repr__(&self) -> String { format!("<VID({:?})>", self.vid) }}

#[pymethods]
impl PyAST {
  #[new] fn __new__()->Self { Self{ base: ASTBase::empty() }}
  fn op_and(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID{ nid:self.base.and(x.nid, y.nid) }}
  fn op_xor(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID{ nid:self.base.xor(x.nid, y.nid) }}
  fn op_or(&mut self, x:&PyNID, y:&PyNID)->PyNID  { PyNID{ nid:self.base.or(x.nid, y.nid) }}
  fn to_dot(&self, x:&PyNID)->String { let mut s = String::new(); self.base.write_dot(x.nid, &mut s); s }}

#[pymodule]
fn bex(py:Python, m:&PyModule)->PyResult<()> {
  m.add_class::<PyVID>()?;
  m.add_class::<PyNID>()?;
  m.add_class::<PyAST>()?;
  m.setattr("O", PyNID{nid:O}.into_py(py))?;
  m.setattr("I", PyNID{nid:I}.into_py(py))?;

  #[pyfn(m, "var")] fn var(_py:Python, i:i32)->PyResult<PyNID> { PyNID::var(i) }
  #[pyfn(m, "vir")] fn vir(_py:Python, i:i32)->PyResult<PyNID> { PyNID::vir(i) }

  Ok(())}
