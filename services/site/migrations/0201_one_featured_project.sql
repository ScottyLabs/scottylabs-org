-- One project is featured on the Projects page at a time (spec P-1). The API clears the others
-- when it features one; this index makes sure two admins featuring different projects at the
-- same moment cannot both win.
CREATE UNIQUE INDEX projects_one_featured ON projects ((true)) WHERE featured;
