import { useState, useRef, useEffect } from "react";
import useWebSocket from "react-use-websocket";
import { useDebouncedCallback } from "use-debounce";

import init, { to_sql, InitOutput } from "sqlsonnet";
import "./App.css";
import logo from "./../../logo.png";

import "bootstrap/dist/css/bootstrap.min.css";

import initial from "./initial.jsonnet?raw";

// Codemirror
import "codemirror/lib/codemirror.css";
import "codemirror/mode/sql/sql.js";
// @ts-ignore
import { jsonnet } from "./jsonnet.js";
import { UnControlled as CodeMirror } from "react-codemirror2";

type Location = [line: number, col: number];

interface WsResponse {
  sql?: string;
  data?: string;
  initial?: string;
  share?: string;
  error?: string;
}

const proxy = import.meta.env.VITE_PROXY == "1";
const websocket = import.meta.env.VITE_WEBSOCKET || "/play/ws";
const debounce_ms = 200;

function Editor({
  value,
  onChange = () => {},
  onKeyUp = () => {},
  mode,
  readOnly = false,
  location = null,
}: {
  value: string;
  onChange?: (data: string) => void;
  onKeyUp?: (value: string, event: KeyboardEvent) => void;
  mode: string;
  readOnly?: boolean;
  location?: Location | null;
}) {
  // https://github.com/scniro/react-codemirror2/issues/284
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
      onKeyUp={(editor, event) => {
        onKeyUp(editor.getValue(), event);
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

function Alert({ value }: { value: JSX.Element | null }) {
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
  const [alert, setAlert] = useState(null as JSX.Element | null);
  const [sql, setSql] = useState("");
  const [jsonnet, setJsonnet] = useState("");
  // We need to use another state because the Controlled version of the editor has a bug
  // with React 18, and updating the value on change of the Uncontrolled version does not work.
  // https://github.com/scniro/react-codemirror2/issues/313
  const [jsonnet2, setJsonnet2] = useState("");
  const [shareLink, setShareLink] = useState("");
  const [data, setData] = useState("");
  const [location, setLocation] = useState(null);

  const share = new URLSearchParams(window.location.search).get("share");

  const { sendJsonMessage, lastJsonMessage } = useWebSocket(
    websocket + (share ? "?share=" + share : ""),
    {
      share: false,
      shouldReconnect: () => true,
    },
  );
  const sendJsonMessageDeb = useDebouncedCallback(sendJsonMessage, debounce_ms);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const setError = (error: any) => {
    if (typeof error != "object") {
      setAlert(error.toString());
    }
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
  };

  const refresh = (data: string) => {
    setJsonnet2(data);
    setAlert(null);
    setLocation(null);
    setData("");
    setShareLink("");
    if (proxy) {
      sendJsonMessageDeb({ jsonnet: data });
    } else {
      getWasm().then(() => {
        try {
          setSql(to_sql(data));
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
        } catch (error: any) {
          setError(error);
        }
      });
    }
  };

  // Receive new messages
  useEffect(() => {
    const response = lastJsonMessage as WsResponse | null;
    if (response == null) {
      return;
    }
    if (response.error == null) {
      if (response.sql != null) {
        setSql(response.sql);
      }
      if (response.initial != null) {
        setJsonnet(response.initial);
        sendJsonMessage({ jsonnet: response.initial, clickhouse: true });
      }
      if (response.data != null) {
        setData(response.data);
      }
      if (response.share != null) {
        const location = window.location;
        setShareLink(
          location.protocol +
            "//" +
            location.host +
            location.pathname +
            "?share=" +
            response.share,
        );
      }
    } else {
      setError(response.error);
    }
  }, [lastJsonMessage, sendJsonMessage]);

  useEffect(() => {
    if (proxy) {
      return;
    }
    setJsonnet(initial);
    refresh(initial);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <>
      <div className="row mb-2">
        <h1 className="mt-2">
          <img
            style={{ height: "50px" }}
            alt="sqlsonnet"
            title="sqlsonnet"
            src={logo}
          />{" "}
          playground
        </h1>
        {proxy ? (
          <p>
            The Jsonnet input (left) is sent to the server via a{" "}
            <a href="https://en.wikipedia.org/wiki/WebSocket">WebSocket</a>,
            which returns the generated SQL (right), using the embedded library.
            The prelude is prepended to the input automatically. When desired,
            the query can be send for execution to the Clickhouse server.
          </p>
        ) : (
          <p>
            This demo runs in your browser, using a{" "}
            <a href="https://en.wikipedia.org/wiki/WebAssembly">WebAssembly</a>{" "}
            build of <a href="https://github.com/cpg314/sqlsonnet">sqlsonnet</a>
            .
          </p>
        )}
      </div>
      <div className="row">
        <div className="col-6">
          <Editor
            mode="jsonnet"
            value={jsonnet}
            onChange={refresh}
            onKeyUp={(value, event) => {
              if (!proxy) {
                return;
              }
              if (
                (event.keyCode == 10 || event.keyCode == 13) &&
                event.ctrlKey
              ) {
                sendJsonMessage({ jsonnet: value, clickhouse: true });
              }
            }}
            location={location}
          />
          <p>Input jsonnet</p>
        </div>
        <div className="col-6">
          <Editor value={sql} mode="sql" readOnly={true} />
          <p>Generated SQL</p>
        </div>
      </div>
      {proxy ? (
        <div className="row mt-2">
          <form>
            <button
              onClick={(e: React.MouseEvent<HTMLElement>) => {
                e.preventDefault();
                sendJsonMessage({ jsonnet: jsonnet2, clickhouse: true });
              }}
              className="btn btn-primary me-2"
            >
              Submit to Clickhouse
            </button>
            <a
              href=""
              onClick={(e) => {
                e.preventDefault();
                sendJsonMessage({ jsonnet: jsonnet2, share: true });
              }}
              id="share"
              className="btn btn-secondary btn-sm"
            >
              Share
            </a>
            <div className="form-text">
              {shareLink.length > 0 ? (
                <a href={shareLink}>{shareLink}</a>
              ) : (
                <></>
              )}
            </div>
            <div className="form-text">
              A return limit of 20 rows is automatically added. Type Control +
              Enter to submit.
            </div>
          </form>
        </div>
      ) : (
        <></>
      )}
      <div className="row mt-2">
        <div className="col-12">
          <Alert value={alert} />
        </div>
      </div>
      <div className="row mt-2">
        <div className="col-12">
          {data.length > 0 ? <pre className="data">{data}</pre> : <></>}
        </div>
      </div>
    </>
  );
}

export default App;
