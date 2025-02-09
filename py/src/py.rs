//! wrap bex as a python module
extern crate bex as bex_rs;
use pyo3::prelude::*;
use pyo3::exceptions::PyException;
use bex_rs::{Base, GraphViz, ast::ASTBase, nid::{I,O,NID}, vid::VID};

#[pyclass(name="NID")] struct PyNID(NID);
#[pyclass(name="VID")] struct PyVID(VID);
#[pyclass(name="ASTBase")] struct PyASTBase(ASTBase);

enum BexErr { NegVar, NegVir }
impl std::convert::From<BexErr> for PyErr {
  fn from(err: BexErr) -> PyErr {
    match err {
      BexErr::NegVar => PyException::new_err("var(i) expects i >= 0"),
      BexErr::NegVir => PyException::new_err("vir(i) expects i >= 0") }}}

#[pymethods]
impl PyNID {
  fn is_const(&self)->bool { self.0.is_const() }
  fn __str__(&self) -> String { self.0.to_string() }
  fn __repr__(&self) -> String { format!("<NID({:?})>", self.0) }}

#[pymethods]
impl PyVID {
  fn __str__(&self) -> String { self.0.to_string() }
  fn __repr__(&self) -> String { format!("<VID({:?})>", self.0) }}

#[pymethods]
impl PyASTBase {
  #[new] fn __new__()->Self { Self(ASTBase::empty()) }
  fn op_and(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID(self.0.and(x.0, y.0)) }
  fn op_xor(&mut self, x:&PyNID, y:&PyNID)->PyNID { PyNID(self.0.xor(x.0, y.0)) }
  fn op_or(&mut self, x:&PyNID, y:&PyNID)->PyNID  { PyNID(self.0.or(x.0, y.0)) }
  fn to_dot(&self, x:&PyNID)->String { let mut s = String::new(); self.0.write_dot(x.0, &mut s); s }}

#[pyfunction]
fn var(i:i32)->PyResult<PyNID> { if i<0 { Err(BexErr::NegVar.into()) } else { Ok(PyNID(NID::var(i as u32))) }}
#[pyfunction]
fn vir(i:i32)->PyResult<PyNID> { if i<0 { Err(BexErr::NegVir.into()) } else { Ok(PyNID(NID::vir(i as u32))) }}

#[pymodule]
fn bex(m: &Bound<'_, PyModule>)->PyResult<()> {
  m.add_class::<PyVID>()?;
  m.add_class::<PyNID>()?;
  m.add_class::<PyASTBase>()?;
  m.add("O", PyNID(O))?;
  m.add("I", PyNID(I))?;

  m.add_function(wrap_pyfunction!(var, m)?)?;
  m.add_function(wrap_pyfunction!(vir, m)?)?;

  Ok(())}
