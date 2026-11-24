# Seed images

Images bundled in `sites/web/static/seed/` for seeded content, and the Figma image hash that each one has in the design file (file key `8EbIg2lNqqNl9nhAxk8k3Q`). To find which file a Figma image uses, read the node's `fills[0].imageHash` and look it up here. Dimensions are in `services/site/seed/images.json`.

| Figma hash | Seed path |
| -- | -- |
| `f2b484741cae32d29201ca3beb0915ba7a2ff09d` | `/seed/projects/maps.jpg` |
| `295e60fe3aabcb72e4d238090510ab6c1d1c4ff8` | `/seed/projects/eats.jpg` |
| `ed3d0dd850e2aac9d861636e45f76fe72fa767da` | `/seed/projects/housing.jpg` |
| `7f184e6c341df9034c9ef058d6e340e679bd8243` | `/seed/projects/study.jpg` |
| `b2b3e86817913e03049aa7e96cd2ff71e959f82e` | `/seed/projects/bussign.jpg` |
| `574167e20832eb8612b9f151245030070c3a14a6` | `/seed/projects/tartanhacks.jpg` |
| `57c8981cf26130e7022d9759bbc4f24f93b3a3af` | `/seed/projects/vote.jpg` |
| `8ecaaed5d426f95b5d702605b3cbdfb2bcb7ff91` | `/seed/projects/components.jpg` |
| `7d0ab3d5bdf5bb374d36c02b2099f4c26f3cfb88` | `/seed/projects/docs.jpg` |
| `cfff77898bcec5af3cbced6fb4e8fdbe4efc7553` | `/seed/projects/quest.jpg` |
| `1c0c3e4cc4f2a65446ceba23694ebb07aa990dbd` | `/seed/projects/courses.jpg` |
| `da442c6ad2ff0422943048b168048ab0453ebefa` | `/seed/photos/c01.jpg` |
| `8cddd72a73dabc9ae0a68ccfef2dc4a74d83597d` | `/seed/photos/c04.jpg` |
| `f6b046f960f264b0369f3904712de563af67c8ec` | `/seed/photos/c05.jpg` |
| `d906143d426f4f3cd5fe3792d8f5cefd2403cab4` | `/seed/photos/c09.jpg` |
| `f86c2e9a7a1f3b34672d6826fd7454d39a72d52a` | `/seed/photos/c11.jpg` |
| `d56fb8d888c41b82d87355bacd68b5a5cc0687ae` | `/seed/photos/c13.jpg` |
| `43b8f32e35218afd7fcf3645903f0dd71cadaa04` | `/seed/photos/c14.jpg` |
| `06cb9881b4056c5cba841853593be2d9a8b573ed` | `/seed/photos/c15.jpg` |
| `caac1d49770733f9f0a97ff6c029d459efa8bc5a` | `/seed/photos/c02.jpg` |
| `ded605422f98306fa2a739b6b0bb29a94d6c491d` | `/seed/photos/c10.jpg` |
| `d783ef542ae26864c8e9e1dc5ffd931b374e1030` | `/seed/photos/t07.jpg` |
| `20b5a0925bdcf762c14a7fc040c87a03516bfe77` | `/seed/photos/t09.jpg` |
| `8a8a6c1da6b15ae3e2c98f8bc7b4aa419a33fb69` | `/seed/photos/t02.jpg` |
| `f1cf650ac65ff3de3a24d4ac090761ab6871fede` | `/seed/photos/c16.jpg` |
| `51b0975bfd010a7a6b0d770f3cac384d867af43a` | `/seed/photos/c03.jpg` |
| `75a4d075bcb8efa8be6ccdcbd19629e169bfa0d3` | `/seed/photos/c06.jpg` |
| `d47f2e84a787fa0535a575a01bb7d7b5529168fa` | `/seed/photos/c07.jpg` |
| `e9bd131e5968c1ee737f97298f42121fa32e0488` | `/seed/photos/c12.jpg` |
| `5ef138f768f153b7c4dadb26ba2e818c44c7d94c` | `/seed/photos/t01.jpg` |
| `f828d9732eceade0677d53bd431b32b2fd2e0e7d` | `/seed/photos/t03.jpg` |
| `44022709140ac5b5259421825ac22f7d74913d51` | `/seed/photos/t04.jpg` |
| `be246864ecb5599e76272f707e407629a6fbc198` | `/seed/photos/t05.jpg` |
| `9dd34afb5692e9e3b754719d7072dab2b3b623b8` | `/seed/photos/t11.jpg` |
| `096b900acb89b83449a2c6ba625fbf467b60297c` | `/seed/photos/t12.jpg` |

Where the prototype used them:

- Committee covers: events `c04`, outreach `c02`, tech `c10`, labrador `c13`, design `c05`, finance `t04`, foundry `c07` (all in `/seed/photos/`).
- Project screenshots: `/seed/projects/<name>.jpg` for CMU Maps (`maps`), CMU Eats (`eats`), CMU Housing (`housing`), CMU Study (`study`), Bus Sign (`bussign`), TartanHacks (`tartanhacks`), Tartan Vote (`vote`), ScottyLabs Components (`components`), ScottyLabs Docs (`docs`), Orientation Quest (`quest`), CMU Courses (`courses`).
