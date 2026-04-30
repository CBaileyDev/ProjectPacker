import { HashRouter, Link, Route, Routes } from "react-router-dom";
import Home from "./routes/Home";
import Pack from "./routes/Pack";
import Result from "./routes/Result";
import Bridge from "./routes/Bridge";

export default function App() {
  return (
    <HashRouter>
      <div className="flex h-full flex-col">
        <nav className="border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-sm">
          <Link className="mr-4 underline" to="/">Home</Link>
          <Link className="mr-4 underline" to="/pack">Pack</Link>
          <Link className="mr-4 underline" to="/bridge">Bridge</Link>
        </nav>
        <main className="flex-1 overflow-auto p-4">
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/pack" element={<Pack />} />
            <Route path="/result" element={<Result />} />
            <Route path="/bridge" element={<Bridge />} />
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
