import { useMemo, useState } from "react";
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  List,
  ListItemButton,
  ListItemText,
  Stack,
  Typography,
} from "@mui/material";
import FolderRoundedIcon from "@mui/icons-material/FolderRounded";
import { CheckTile, IconTile, SearchField } from "@/components/ui";
import { HostAvatar } from "@/hosts/HostAvatar";
import type { GroupNode, HostCard, Uuid } from "@/ipc/types";
import { sizes } from "@/theme/theme";

/** Ids of `group` and every group nested inside it. */
function subtree(groups: readonly GroupNode[], group: Uuid): Set<Uuid> {
  const out = new Set<Uuid>([group]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const g of groups) {
      if (g.parentId && out.has(g.parentId) && !out.has(g.id)) {
        out.add(g.id);
        grew = true;
      }
    }
  }
  return out;
}

/** Hosts inside `group` (recursively). */
export function groupHosts(
  hosts: readonly HostCard[],
  groups: readonly GroupNode[],
  group: Uuid,
): HostCard[] {
  const ids = subtree(groups, group);
  return hosts.filter((h) => h.groupId !== null && ids.has(h.groupId));
}

/**
 * Pick hosts (and whole groups) a snippet should run on. Toggling a group
 * toggles every host inside it, so the selection is always a plain host set.
 */
export function TargetsDialog({
  hosts,
  groups,
  initial,
  busy,
  onCancel,
  onConfirm,
}: {
  hosts: readonly HostCard[];
  groups: readonly GroupNode[];
  initial: readonly Uuid[];
  busy: boolean;
  onCancel: () => void;
  onConfirm: (hostIds: Uuid[]) => void;
}) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<Uuid>>(() => new Set(initial));
  const q = query.trim().toLowerCase();

  const visibleHosts = useMemo(
    () =>
      hosts.filter(
        (h) =>
          !q ||
          h.label.toLowerCase().includes(q) ||
          h.address.toLowerCase().includes(q) ||
          h.groupPath.some((p) => p.toLowerCase().includes(q)),
      ),
    [hosts, q],
  );
  const visibleGroups = useMemo(
    () =>
      groups
        .map((g) => ({ group: g, members: groupHosts(hosts, groups, g.id) }))
        .filter(
          ({ group, members }) =>
            members.length > 0 && (!q || group.label.toLowerCase().includes(q)),
        ),
    [groups, hosts, q],
  );

  const toggleHost = (id: Uuid) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const toggleGroup = (members: HostCard[]) =>
    setSelected((s) => {
      const n = new Set(s);
      const all = members.every((h) => n.has(h.id));
      for (const h of members) {
        if (all) n.delete(h.id);
        else n.add(h.id);
      }
      return n;
    });

  // Keep the previous order for hosts that stay selected; append new ones in list order.
  const result = () => {
    const kept = initial.filter((id) => selected.has(id));
    const added = hosts.filter((h) => selected.has(h.id) && !kept.includes(h.id)).map((h) => h.id);
    return [...kept, ...added];
  };

  return (
    <Dialog open onClose={busy ? undefined : onCancel} maxWidth="xs" fullWidth>
      <DialogTitle>Add targets</DialogTitle>
      <DialogContent sx={{ display: "flex", flexDirection: "column", gap: 1.5, pb: 0 }}>
        <SearchField
          value={query}
          onChange={setQuery}
          placeholder="Search hosts and groups"
          width="100%"
          autoFocus
        />
        {hosts.length === 0 ? (
          <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>
            No hosts yet. Add hosts first, then pick them here.
          </Typography>
        ) : (
          <Stack sx={{ maxHeight: 380, overflowY: "auto", mx: -1 }}>
            {visibleGroups.length > 0 && (
              <>
                <Typography variant="caption" color="text.secondary" sx={{ px: 1, pt: 0.5 }}>
                  Groups
                </Typography>
                <List dense disablePadding>
                  {visibleGroups.map(({ group, members }) => {
                    const picked = members.filter((h) => selected.has(h.id)).length;
                    return (
                      <ListItemButton
                        key={group.id}
                        onClick={() => toggleGroup(members)}
                        sx={{ borderRadius: 1.5, gap: 1 }}
                      >
                        <CheckTile
                          size={sizes.tileSmall}
                          checked={picked === members.length}
                          partial={picked > 0 && picked < members.length}
                          tile={
                            <IconTile size={sizes.tileSmall}>
                              <FolderRoundedIcon />
                            </IconTile>
                          }
                        />
                        <ListItemText
                          primary={group.label}
                          secondary={`${members.length} ${members.length === 1 ? "host" : "hosts"}`}
                          slotProps={{ primary: { noWrap: true }, secondary: { noWrap: true } }}
                        />
                      </ListItemButton>
                    );
                  })}
                </List>
              </>
            )}
            <Typography variant="caption" color="text.secondary" sx={{ px: 1, pt: 0.5 }}>
              Hosts
            </Typography>
            {visibleHosts.length === 0 ? (
              <Typography variant="body2" color="text.secondary" sx={{ px: 1, py: 1 }}>
                Nothing matches “{query}”.
              </Typography>
            ) : (
              <List dense disablePadding>
                {visibleHosts.map((h) => (
                  <ListItemButton
                    key={h.id}
                    onClick={() => toggleHost(h.id)}
                    sx={{ borderRadius: 1.5, gap: 1 }}
                  >
                    <CheckTile
                      size={sizes.tileSmall}
                      checked={selected.has(h.id)}
                      tile={<HostAvatar host={h} size={sizes.tileSmall} />}
                    />
                    <ListItemText
                      primary={h.label}
                      secondary={[
                        ...h.groupPath,
                        h.username ? `${h.username}@${h.address}` : h.address,
                      ].join(" › ")}
                      slotProps={{ primary: { noWrap: true }, secondary: { noWrap: true } }}
                    />
                  </ListItemButton>
                ))}
              </List>
            )}
          </Stack>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2, pt: 1.5 }}>
        <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }}>
          {selected.size} selected
        </Typography>
        <Button onClick={onCancel} disabled={busy} color="inherit">
          Cancel
        </Button>
        <Button variant="contained" disabled={busy} onClick={() => onConfirm(result())}>
          Save targets
        </Button>
      </DialogActions>
    </Dialog>
  );
}
