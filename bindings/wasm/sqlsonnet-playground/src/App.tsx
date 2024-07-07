import { useState, useRef, useEffect } from "react";

import init, { to_sql, InitOutput } from "sqlsonnet";
import "./App.css";

import "bootstrap/dist/css/bootstrap.min.css";

import initial from "./initial.jsonnet?raw";

import "codemirror/lib/codemirror.css";
import "codemirror/mode/sql/sql.js";
// @ts-ignore
import { jsonnet } from "./jsonnet.js";
import { UnControlled as CodeMirror } from "react-codemirror2";

function Editor({
  value,
  onChange = (_data) => {},
  mode,
  readOnly = false,
}: {
  value: string;
  onChange?: (data: string) => void;
  mode: string;
  readOnly?: boolean;
}) {
  const editor = useRef();
  const wrapper = useRef();
  const editorWillUnmount = () => {
    if (editor.current) {
      // @ts-ignore
      editor.current.display.wrapper.remove();
    }
    if (wrapper.current) {
      // @ts-ignore
      wrapper.current.hydrated = false;
    }
  };
  return (
    <CodeMirror
      value={value}
      defineMode={{ name: "jsonnet", fn: jsonnet }}
      options={{
        mode: mode,
        lineNumbers: true,
        lineWrapping: true,
        readOnly: readOnly,
      }}
      onChange={(_editor, _data, value) => {
        onChange(value);
      }}
      // @ts-ignore
      ref={wrapper}
      editorDidMount={(e) => (editor.current = e)}
      editorWillUnmount={editorWillUnmount}
    />
  );
}

function Alert({ value }: { value: string }) {
  return value.length == 0 ? (
    <></>
  ) : (
    <div className="alert alert-warning" role="alert">
      {value}
    </div>
  );
}

let wasmPromise: Promise<InitOutput> | null = null;
export function getWasm() {
  if (!wasmPromise) {
    wasmPromise = init();
  }
  return wasmPromise;
}

function App() {
  const [alert, setAlert] = useState("");
  const [valueSql, setValueSql] = useState("");

  const refresh = (data: string) => {
    setAlert("");
    getWasm().then(() => {
      try {
        setValueSql(to_sql(data));
      } catch (error: any) {
        setAlert(error.toString());
      }
    });
  };

  useEffect(() => {
    refresh(initial);
  }, []);

  return (
    <>
      <div className="row">
        <div className="col-6">
          <Editor value={initial} mode="jsonnet" onChange={refresh} />
          <p>Input jsonnet</p>
        </div>
        <div className="col-6">
          <Editor value={valueSql} mode="sql" readOnly={true} />
          <p>Generated SQL</p>
        </div>
      </div>
      <div className="row mt-2">
        <Alert value={alert} />
      </div>
    </>
  );
}

export default App;
