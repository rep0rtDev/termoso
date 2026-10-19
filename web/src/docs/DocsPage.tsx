import { useEffect, useMemo, useState } from "react";
import { Box, Button, Divider, Link, Typography, alpha } from "@mui/material";
import ArrowBackRoundedIcon from "@mui/icons-material/ArrowBackRounded";
import ArrowForwardRoundedIcon from "@mui/icons-material/ArrowForwardRounded";
import EditOutlinedIcon from "@mui/icons-material/EditOutlined";
import { Link as RouterLink, useLocation, useParams } from "react-router";
import { emerald, monoFontFamily } from "@/theme/theme";
import { NotFoundPage } from "@/pages/NotFoundPage";
import { findPage, hrefOf, pages, type DocPage } from "./manifest";
import { renderMarkdown, type TocEntry } from "./markdown";
import { DOCS_HEADER, REPO } from "./DocsLayout";

const markdownSx = {
  fontSize: "1rem",
  lineHeight: 1.7,
  color: "text.primary",
  "& > :first-of-type": { mt: 0 },
  "& h2, & h3, & h4": {
    position: "relative",
    fontWeight: 600,
    letterSpacing: "-0.01em",
    scrollMarginTop: DOCS_HEADER + 24,
    "& .anchor": {
      position: "absolute",
      left: -24,
      width: 24,
      textAlign: "center",
      color: "text.disabled",
      textDecoration: "none",
      opacity: 0,
    },
    "&:hover .anchor": { opacity: 1 },
  },
  "& h2": { fontSize: "1.5rem", mt: 6, mb: 2, pt: 3, borderTop: 1, borderColor: "border.light" },
  "& h3": { fontSize: "1.1875rem", mt: 4, mb: 1.5 },
  "& h4": { fontSize: "1rem", mt: 3, mb: 1 },
  "& p, & ul, & ol": { my: 1.5 },
  "& li + li": { mt: 0.5 },
  "& li > p": { my: 0.5 },
  "& a": { color: "primary.main", textDecorationColor: alpha(emerald.main, 0.4) },
  "& strong": { fontWeight: 600 },
  "& code": {
    fontFamily: monoFontFamily,
    fontSize: "0.875em",
    px: 0.6,
    py: 0.1,
    borderRadius: 1,
    bgcolor: "surface.high",
    border: 1,
    borderColor: "border.light",
  },
  "& pre": {
    my: 2,
    p: 2,
    borderRadius: 2,
    bgcolor: "surface.base",
    border: 1,
    borderColor: "border.light",
    overflowX: "auto",
    fontSize: "0.8125rem",
    lineHeight: 1.6,
    "& code": { p: 0, border: 0, bgcolor: "transparent", fontSize: "inherit" },
  },
  "& blockquote": {
    my: 2,
    mx: 0,
    pl: 2,
    borderLeft: 3,
    borderColor: "border.strong",
    color: "text.secondary",
  },
  "& .callout": {
    my: 2.5,
    px: 2.5,
    py: 1.5,
    borderRadius: 2,
    border: 1,
    borderColor: "border.light",
    bgcolor: "surface.base",
    "& p": { my: 1 },
    "& .callout-title": { fontWeight: 600, fontSize: "0.875rem", mb: 0.5 },
  },
  "& .callout-tip, & .callout-note": {
    "& .callout-title": { color: "primary.main" },
    borderColor: alpha(emerald.main, 0.35),
  },
  "& .callout-warning, & .callout-important": {
    "& .callout-title": { color: "warning.main" },
    borderColor: "warning.main",
  },
  "& figure": {
    my: 3,
    mx: 0,
    "& img": {
      display: "block",
      width: "100%",
      borderRadius: 2,
      border: 1,
      borderColor: "border.basic",
      boxShadow: "0 20px 50px -20px rgba(0,0,0,0.5)",
      "&.portrait": { width: "100%", maxWidth: 320, mx: "auto" },
    },
    "& figcaption": {
      mt: 1,
      textAlign: "center",
      fontSize: "0.8125rem",
      color: "text.secondary",
    },
  },
  "& table": {
    my: 2,
    width: "100%",
    borderCollapse: "collapse",
    fontSize: "0.9375rem",
    display: "block",
    overflowX: "auto",
    "& th, & td": {
      px: 1.5,
      py: 1,
      borderBottom: 1,
      borderColor: "border.light",
      textAlign: "left",
      verticalAlign: "top",
    },
    "& th": { fontWeight: 600, color: "text.secondary", fontSize: "0.8125rem" },
    "& th:first-of-type, & td:first-of-type": { whiteSpace: "nowrap" },
    "& td": { minWidth: 120 },
    "& code": { overflowWrap: "anywhere" },
    "& kbd": { whiteSpace: "nowrap" },
  },
  "& hr": { my: 4, border: 0, borderTop: 1, borderColor: "border.light" },
  "& kbd": {
    fontFamily: "inherit",
    fontSize: "0.8125em",
    px: 0.75,
    py: 0.1,
    borderRadius: 1,
    border: 1,
    borderColor: "border.basic",
    bgcolor: "surface.high",
  },
} as const;

function useActiveHeading(toc: TocEntry[]): string | null {
  const [active, setActive] = useState<string | null>(null);
  useEffect(() => {
    if (toc.length === 0) return;
    const els = toc
      .map((t) => document.getElementById(t.id))
      .filter((e): e is HTMLElement => e !== null);
    const onScroll = () => {
      const line = DOCS_HEADER + 40;
      let current: string | null = els[0]?.id ?? null;
      for (const el of els) {
        if (el.getBoundingClientRect().top <= line) current = el.id;
        else break;
      }
      setActive(current);
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, [toc]);
  return active;
}

function Toc({ toc }: { toc: TocEntry[] }) {
  const active = useActiveHeading(toc);
  if (toc.length === 0) return null;
  return (
    <Box
      component="aside"
      sx={{
        width: 220,
        flexShrink: 0,
        position: "sticky",
        top: DOCS_HEADER + 32,
        display: { xs: "none", lg: "block" },
        pl: 3,
        borderLeft: 1,
        borderColor: "border.light",
      }}
    >
      <Typography
        variant="overline"
        component="div"
        sx={{ color: "text.secondary", letterSpacing: "0.08em", mb: 1 }}
      >
        On this page
      </Typography>
      {toc.map((t) => (
        <Link
          key={t.id}
          href={`#${t.id}`}
          underline="none"
          variant="body2"
          sx={{
            display: "block",
            py: 0.5,
            pl: t.depth === 3 ? 1.5 : 0,
            color: active === t.id ? "primary.main" : "text.secondary",
            fontWeight: active === t.id ? 600 : 400,
            "&:hover": { color: "text.primary" },
          }}
        >
          {t.text}
        </Link>
      ))}
    </Box>
  );
}

function PagerLink({ page, next }: { page: DocPage; next?: boolean }) {
  return (
    <Button
      component={RouterLink}
      to={hrefOf(page)}
      variant="outlined"
      color="inherit"
      sx={{
        flex: 1,
        justifyContent: next ? "flex-end" : "flex-start",
        textAlign: next ? "right" : "left",
        borderColor: "border.light",
        px: 2,
        py: 1.5,
        display: "flex",
        gap: 1.5,
        alignItems: "center",
        textTransform: "none",
      }}
    >
      {!next && <ArrowBackRoundedIcon fontSize="small" sx={{ color: "text.secondary" }} />}
      <Box>
        <Typography variant="caption" color="text.secondary" component="div">
          {next ? "Next" : "Previous"}
        </Typography>
        <Typography variant="body2" sx={{ fontWeight: 600 }} component="div">
          {page.title}
        </Typography>
      </Box>
      {next && <ArrowForwardRoundedIcon fontSize="small" sx={{ color: "text.secondary" }} />}
    </Button>
  );
}

export function DocsPage() {
  const { "*": splat } = useParams();
  const location = useLocation();
  const page = findPage(splat ?? "");
  const rendered = useMemo(() => (page ? renderMarkdown(page.body) : null), [page]);

  useEffect(() => {
    if (!page) return;
    document.title = `${page.title} · Termoso docs`;
    return () => {
      document.title = "Termoso";
    };
  }, [page]);

  useEffect(() => {
    if (location.hash) {
      const el = document.getElementById(decodeURIComponent(location.hash.slice(1)));
      if (el) {
        el.scrollIntoView();
        return;
      }
    }
    window.scrollTo(0, 0);
  }, [location.pathname, location.hash, rendered]);

  if (!page || !rendered) return <NotFoundPage to="/docs" label="Open the docs" />;
  const i = pages.indexOf(page);
  const prev = i > 0 ? pages[i - 1] : null;
  const next = i < pages.length - 1 ? pages[i + 1] : null;
  const source = `${REPO}/edit/main/web/src/docs/content/${page.path || "index"}.md`;

  return (
    <Box
      sx={{
        display: "flex",
        alignItems: "flex-start",
        gap: 6,
        px: { xs: 0, md: 6 },
        py: { xs: 3, md: 5 },
      }}
    >
      <Box
        component="article"
        sx={{ flex: 1, minWidth: 0, maxWidth: 780, overflowWrap: "anywhere" }}
      >
        <Typography
          variant="overline"
          component="div"
          color="primary.main"
          sx={{ letterSpacing: "0.1em", mb: 1 }}
        >
          {page.section}
        </Typography>
        <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
          <Box
            sx={{
              width: 44,
              height: 44,
              borderRadius: 2,
              display: { xs: "none", sm: "grid" },
              placeItems: "center",
              bgcolor: alpha(emerald.main, 0.12),
              color: "primary.main",
              flexShrink: 0,
              "& svg": { fontSize: 24 },
            }}
          >
            {page.icon}
          </Box>
          <Typography
            variant="h1"
            component="h1"
            sx={{
              fontSize: { xs: "2rem", md: "2.5rem" },
              fontWeight: 600,
              letterSpacing: "-0.02em",
              lineHeight: 1.15,
            }}
          >
            {page.title}
          </Typography>
        </Box>
        {page.description && (
          <Typography
            variant="body1"
            color="text.secondary"
            sx={{ mt: 2, fontSize: "1.125rem", lineHeight: 1.6 }}
          >
            {page.description}
          </Typography>
        )}
        <Box
          className="docs-md"
          sx={{ ...markdownSx, mt: 4 }}
          dangerouslySetInnerHTML={{ __html: rendered.html }}
        />
        <Divider sx={{ mt: 6, mb: 3 }} />
        <Box
          sx={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            flexWrap: "wrap",
            gap: 2,
          }}
        >
          <Link
            href={source}
            target="_blank"
            rel="noreferrer"
            variant="body2"
            color="text.secondary"
            underline="hover"
            sx={{ display: "inline-flex", alignItems: "center", gap: 0.75 }}
          >
            <EditOutlinedIcon sx={{ fontSize: 16 }} /> Edit this page on GitHub
          </Link>
        </Box>
        <Box sx={{ display: "flex", gap: 2, mt: 3, flexDirection: { xs: "column", sm: "row" } }}>
          {prev ? <PagerLink page={prev} /> : <Box sx={{ flex: 1 }} />}
          {next ? <PagerLink page={next} next /> : <Box sx={{ flex: 1 }} />}
        </Box>
      </Box>
      <Toc toc={rendered.toc} />
    </Box>
  );
}
