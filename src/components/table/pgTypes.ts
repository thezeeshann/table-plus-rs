/** Common PostgreSQL column types offered in the structure editor (TablePlus lists the same core set). */
export const PG_TYPES = [
  "bigint", "bigserial", "bit", "boolean", "box", "bytea", "character", "character varying", "varchar(255)", "cidr", "circle",
  "date", "double precision", "inet", "integer", "interval", "json", "jsonb", "line", "lseg", "macaddr", "money",
  "numeric", "numeric(10,2)", "path", "point", "polygon", "real", "serial", "smallint", "smallserial", "text", "time",
  "time with time zone", "timestamp", "timestamp with time zone", "timestamptz", "tsquery", "tsvector", "uuid", "xml",
  "integer[]", "text[]", "uuid[]", "jsonb[]",
];

export const FK_ACTIONS = ["NO ACTION", "RESTRICT", "CASCADE", "SET NULL", "SET DEFAULT"];
export const INDEX_METHODS = ["btree", "hash", "gin", "gist", "brin", "spgist"];

export function isNumericType(t: string): boolean {
  return /^(int|float|numeric|decimal|real|double|serial|bigserial|smallserial|money|oid)/.test(t);
}
export function isBoolType(t: string): boolean { return t === "bool" || t === "boolean"; }
export function isDateType(t: string): boolean { return /^(date|time|timestamp)/.test(t); }
export function isJsonType(t: string): boolean { return t === "json" || t === "jsonb"; }
