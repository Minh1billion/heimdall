import { PanelLeft } from "lucide-react";
import { NavLink, useLocation } from "react-router-dom";
import { api } from "../api";
import { useAsync } from "../hooks";
import { NAV, type NavGroup } from "../nav";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const GROUPS: NavGroup[] = ["Workspace", "Ads", "System"];

export function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const location = useLocation();
  const approvals = useAsync(() => api.approvals("pending"), [location.pathname]);
  const pending = approvals.data?.pending ?? 0;

  return (
    <aside className="sidebar">
      <div className="sidebar-top">
        <div className="brand">
          <svg className="brand-mark" viewBox="0 0 32 32" aria-hidden="true">
            <rect width="32" height="32" rx="8" fill="#0d0d0d" />
            <rect x="8" y="7" width="3.4" height="18" rx="1.7" fill="#fff" />
            <rect x="20.6" y="7" width="3.4" height="18" rx="1.7" fill="#fff" />
            <path d="M10 20Q16 9.5 22 20" fill="none" stroke="#fff" strokeWidth="3" strokeLinecap="round" />
          </svg>
          <span className="brand-text">Heimdall</span>
        </div>
        <button
          type="button"
          className="icon-button toggle"
          onClick={onToggle}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <PanelLeft size={20} />
        </button>
      </div>
      <nav className="nav" aria-label="Main">
        {GROUPS.map((group) => (
          <div key={group} className="nav-group">
            <div className="nav-group-label">{group}</div>
            {NAV.filter((item) => item.group === group).map((item) => {
              const Icon = item.icon;
              const count = item.to === "/approvals" ? pending : 0;
              return (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.to === "/"}
                  title={item.label}
                  className={({ isActive }) =>
                    ["nav-item", isActive ? "active" : "", item.ready ? "" : "dim"].join(" ").trim()
                  }
                >
                  <Icon size={20} />
                  <span className="label">{item.label}</span>
                  {!item.ready && <span className="pill">Soon</span>}
                  {count > 0 && <span className="count">{count}</span>}
                </NavLink>
              );
            })}
          </div>
        ))}
      </nav>
      <div className="sidebar-user">
        <span className="avatar">A</span>
        <div className="user-meta">
          <div>Local workspace</div>
          <div className="muted">Single user</div>
        </div>
      </div>
    </aside>
  );
}
