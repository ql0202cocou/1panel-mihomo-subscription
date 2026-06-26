import { Route, Routes } from "react-router-dom";
import AppLayout from "./components/AppLayout";
import RequireAuth from "./components/RequireAuth";
import Login from "./pages/Login";
import ProfileList from "./pages/ProfileList";
import ProfileDetail from "./pages/ProfileDetail";
import GlobalNodes from "./pages/GlobalNodes";
import Settings from "./pages/Settings";

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        element={
          <RequireAuth>
            <AppLayout />
          </RequireAuth>
        }
      >
        <Route path="/" element={<ProfileList />} />
        <Route path="/profiles/:id" element={<ProfileDetail />} />
        <Route path="/nodes" element={<GlobalNodes />} />
        <Route path="/settings" element={<Settings />} />
      </Route>
    </Routes>
  );
}
