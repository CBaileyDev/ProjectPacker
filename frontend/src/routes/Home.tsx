import { Link } from "react-router-dom";

export default function Home() {
  return (
    <div className="space-y-3">
      <h1 className="text-2xl">Home</h1>
      <Link to="/pack" className="inline-block rounded bg-zinc-800 px-3 py-1 hover:bg-zinc-700">New Pack</Link>
      <Link to="/bridge" className="ml-2 inline-block rounded bg-zinc-800 px-3 py-1 hover:bg-zinc-700">Bridge</Link>
    </div>
  );
}
