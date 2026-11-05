import { useState } from "react";
import { Route, Routes } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { WindowBar } from "./components/WindowBar";
import { NAV } from "./nav";
import { AdAccounts } from "./pages/AdAccounts";
import { Alerts } from "./pages/Alerts";
import { Approvals } from "./pages/Approvals";
import { Chat } from "./pages/Chat";
import { ComingSoon } from "./pages/ComingSoon";
import { Destinations } from "./pages/Destinations";
import { Home } from "./pages/Home";
import { Insights } from "./pages/Insights";
import { Logs } from "./pages/Logs";
import { NotFound } from "./pages/NotFound";
import { Reports } from "./pages/Reports";
import { Settings } from "./pages/Settings";
import { Templates } from "./pages/Templates";

export default function App() {
  const [collapsed, setCollapsed] = useState(false);

  return (
    <div className="shell">
      <WindowBar />
      <div className={collapsed ? "app collapsed" : "app"}>
        <Sidebar collapsed={collapsed} onToggle={() => setCollapsed(!collapsed)} />
        <main className="main">
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/destinations" element={<Destinations />} />
            <Route path="/templates" element={<Templates />} />
            <Route path="/reports" element={<Reports />} />
            <Route path="/alerts" element={<Alerts />} />
            <Route path="/chat" element={<Chat />} />
            <Route path="/approvals" element={<Approvals />} />
            <Route path="/logs" element={<Logs />} />
            <Route path="/ad-accounts" element={<AdAccounts />} />
            <Route path="/insights" element={<Insights />} />
            <Route path="/settings" element={<Settings />} />
            {NAV.filter((item) => !item.ready).map((item) => (
              <Route key={item.to} path={item.to} element={<ComingSoon path={item.to} />} />
            ))}
            <Route path="*" element={<NotFound />} />
          </Routes>
        </main>
      </div>
    </div>
  );
}
