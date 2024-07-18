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

type Location = [line: number, col: number];

function Editor({
  value,
  onChange = (_data) => {},
  mode,
  readOnly = false,
  location = null,
}: {
  value: string;
  onChange?: (data: string) => void;
  mode: string;
  readOnly?: boolean;
  location?: Location | null;
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
  if (location) {
    // Set marker
    if (editor.current) {
      // @ts-ignore
      editor.current.markText(
        { line: location[0], ch: location[1] },
        { line: location[0], ch: location[1] + 1 },
        { className: "mark" },
      );
    }
  } else {
    // Unset marker
    if (editor.current) {
      // @ts-ignore
      editor.current.doc.getAllMarks().forEach((marker) => marker.clear());
    }
  }
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

function Alert({ value }: { value: any }) {
  return value == null ? (
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
  const [alert, setAlert] = useState(null as any);
  const [valueSql, setValueSql] = useState("");
  const [location, setLocation] = useState(null);

  const refresh = (data: string) => {
    setAlert(null);
    setLocation(null);
    getWasm().then(() => {
      try {
        setValueSql(to_sql(data));
      } catch (error: any) {
        if (typeof error == "object") {
          if (error.code != null) {
            setAlert(
              <>
                {error.message}
                <br />
                <pre>{error.code}</pre>
              </>,
            );
          } else {
            setAlert(error.message);
          }
          if (error.location) {
            setLocation(error.location);
          }
        } else {
          setAlert(error.toString());
        }
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
          <Editor
            value={initial}
            mode="jsonnet"
            onChange={refresh}
            location={location}
          />
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
