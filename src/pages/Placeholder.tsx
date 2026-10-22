export function Placeholder({ title, note }: { title: string; note: string }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>{title}</h1>
          <p>{note}</p>
        </div>
      </header>
    </div>
  );
}
