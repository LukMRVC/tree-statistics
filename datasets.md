# Information about used datasets

## Dataset info

Here are current used datasets statistics

| Dataset   | Min tree size | Max tree size | Average tree size | Trees  | Avg. unique labels for a tree | Avg. distinct labels in tree | No. of distinct labels | Mean tree height (root to leaf distance) | Tree size/Label ratio | Tree/Label ratio | Max tree height | Min tree height | Max node degree | Min node degree | Mean node degree |
| --------- | ------------- | ------------- | ----------------- | ------ | ----------------------------- | ---------------------------- | ---------------------- | ---------------------------------------- | --------------------- | ---------------- | --------------- | --------------- | --------------- | --------------- | ---------------- |
| Bolzano   | 2             | 2195          | 178.712           | 299    | 2.294                         | 35.916                       | 594                    | 2.841                                    | 30.086                | 0.503            | 3               | 1               | 198             | 1               | 1.989            |
| Ptb       | 3             | 711           | 71.595            | 3832   | 1.747                         | 44.128                       | 13094                  | 6.874                                    | 0.547                 | 0.293            | 29              | 2               | 33              | 1               | 1.972            |
| Sentiment | 2             | 102           | 36.360            | 9645   | 1.040                         | 19.900                       | 19468                  | 7.270                                    | 0.187                 | 0.495            | 29              | 1               | 3               | 1               | 1.945            |
| Treefam   | 63            | 15065         | 2665.613          | 5000   | 223.343                       | 592.227                      | 1276006                | 16.709                                   | 0.209                 | 0.004            | 54              | 2               | 8               | 1               | 1.999            |
| Rna       | 2             | 317           | 94.069            | 37149  | 0.001                         | 15.985                       | 170                    | 6.802                                    | 55.335                | 218.524          | 28              | 1               | 33              | 1               | 1.979            |
| Dblp      | 9             | 1703          | 26.073            | 150000 | 5.814                         | 23.909                       | 992866                 | 2.018                                    | 0.003                 | 0.151            | 5               | 1               | 430             | 1               | 1.923            |
| Python    | 1             | 43270         | 948.242           | 49977  | 15.893                        | 118.270                      | 1479429                | 8.411                                    | 0.064                 | 0.034            | 122             | 0               | 13974           | 0               | 1.998            |
| Swissprot | 63            | 15065         | 2665.613          | 5000   | 223.343                       | 592.227                      | 1276006                | 3.127                                    | 0.209                 | 0.004            | 6               | 1               | 2844            | 1               | 1.995            |

## Dataset sources and info

- Bolzano - Residential addresses in the city of Bolzano.
- DBLP - Bibliographic XML data.
- Python - Abstract syntax trees of Python source code in JSON.
- Sentiment - Semantic trees of movie reviews in the PennTreeBank format.
- Swissprot - Protein sequence data in XML.
- Ptb - PennTreeBank format of texts from Wall Street Journal
- Rna - RNA secondary structures in Homeomorphically Irreducible Tree (HIT) representation. [Dataset from Hugging face](hf://datasets/multimolecule/rnastralign/train.parquet), converted using ViennaRNA
