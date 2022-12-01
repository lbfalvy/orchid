# (c) 2010: Johann A. Briffa <j.briffa@ieee.org>
# $Id: Makefile 1791 2010-09-28 17:00:10Z jabriffa $

TARGETS := main.pdf
DEPENDS := $(wildcard *.tex) $(wildcard *.cls) $(wildcard *.bib)

PDFLATEX=pdflatex

.force: 

all: $(TARGETS)

archive: $(TARGETS)
	rm -f archive.zip
	zip -r archive.zip Figures/ Makefile *.cls *.tex *.bib $(TARGETS) -x "*.svn*"

%.bbl: %.aux
	bibtex $*

%.aux: %.tex $(DEPENDS)
	$(PDFLATEX) $*.tex

%.pdf: %.aux %.bbl
	$(PDFLATEX) $*.tex
	$(PDFLATEX) $*.tex

clean:
	-/bin/rm -f $(TARGETS) *.aux *.log *.bbl *.blg *.out *.toc *.lof *.lot
