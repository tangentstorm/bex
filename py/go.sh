export PY=c:/python36
maturin build -i $PY/python.exe \
&& $PY/scripts/pip install --force-reinstall ../target/wheels/pybex-0.1.5-cp36-none-win_amd64.whl \
&& $PY/scripts/ipython -i test.py
