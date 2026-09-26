//
// Copyright(C) 1993-1996 Id Software, Inc.
// Copyright(C) 2005-2014 Simon Howard
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// DESCRIPTION:
//	BSP traversal, handling of LineSegs for rendering.
//




#include "doomdef.h"

#include "m_bbox.h"

#include "i_system.h"

#include "r_main.h"
#include "r_plane.h"
#include "r_things.h"

// State.
#include "doomstat.h"
#include "r_state.h"

//#include "r_local.h"



/* == INSTRUMENTO (2026-09-03), y se quita cuando conteste ==============
 *
 * `Bad R_RenderWallRange` salta con los numeros al reves, y el metal ya dio
 * DOS pares distintos -- `28 to 27` y `7 to -1`. Lo que NO dice es cual de los
 * OCHO sitios que llaman a `R_StoreWallRange` fue.
 *
 * Esta global lo dice. Se pone justo antes de cada llamada y la lee el
 * RANGECHECK de `r_segs.c`. Los numeros son `1x` en `R_ClipSolidWallSegment`
 * y `2x` en `R_ClipPassWallSegment`; el segundo digito es el orden dentro de
 * la funcion.
 */
int bmo_quien_llamo = 0;
/* == INSTRUMENTO (2026-09-04): LA PREGUNTA QUE DEJO EL SITIO 21 ==========
 *
 * El Ryzen dijo `RANGECHECK sitio 21: start=5 stop=-1`. El sitio 21 es
 * `R_ClipPassWallSegment`, rama "el poste se ve entero", y sus dos argumentos
 * salen de la ultima linea de `R_AddLine`:
 *
 *     R_ClipPassWallSegment (x1, x2 - 1);
 *
 * O sea que llegaron **x1 = 5 y x2 = 0**: x1 MAYOR que x2. Y ahi arriba solo
 * se comprueba `x1 == x2`, nunca `x1 > x2`, porque en el DOOM original no
 * puede pasar: `viewangletox[]` es monotona, asi que angulo mayor da columna
 * menor y el orden se conserva.
 *
 * ** Si aqui pasa, o la tabla no es monotona o los dos angulos entran al
 * reves. Esta linea dice cual, y se calla despues de la primera.
 *
 * == *** Y CALLARSE DESPUES DE LA PRIMERA ERA EL FALLO (2026-09-04) ======
 *
 * Una linea que sale UNA VEZ contesta "ha pasado". No contesta "sigue
 * pasando", que es la unica pregunta que queda cuando ya se ha arreglado
 * algo. **Un aviso que no distingue entre un caso raro y dos mil por
 * fotograma manda a mirar al sitio equivocado las dos veces.**
 *
 * Es la familia de fallo de toda la semana --algo que dice una cosa cierta
 * de una forma que se lee como otra-- y aqui se paga cara: el detalle de
 * abajo se emitio el 04-09, se arreglo el truncado de 32 bits en el
 * compilador, y **la misma linea volveria a salir igual si quedara un solo
 * seg mal de los cuarenta mil de un nivel.**
 *
 * Asi que ahora hay DOS instrumentos y no uno:
 *
 *   - el detalle, una vez, con los numeros de dentro: para diagnosticar.
 *   - los CONTADORES, en el `[perf]`: para saber si sigue vivo.
 *
 * ** `bmo_addline_fuera` es el que de verdad importa, y no mide una
 * consecuencia sino LA CAUSA. `viewangletox` tiene `FINEANGLES/2` = **4096**
 * entradas, y `(angulo + ANG90) >> ANGLETOFINESHIFT` da de 0 a 8191. En el
 * DOOM original el recorte de arriba garantiza que nunca pase de 4095; si
 * pasa, se esta leyendo FUERA de la tabla y lo que salga es basura.
 *
 * El 04-09 salio `fino2=9215`: **5119 enteros pasados del final, 20 KiB de
 * lo que hubiera detras.** Y 9215 no es un numero cualquiera --
 * 9215 = 1023 + 8192, y 8192 * 2^19 es exactamente 2^32: UN acarreo que se
 * escapo de 32 bits y nadie tiro. Ese era el bug del compilador.
 *
 * [!] Con la aritmetica correcta esto tiene que salir **CERO EXACTO**. No
 * "poco": cero. Es la clase de numero que no admite interpretacion, y por eso
 * es el que se mira primero. */
static int bmo_addline_dicho = 0;

/** Cuantos segs entraron al reves desde el ultimo `[perf]`. Consecuencia. */
int bmo_addline_reves = 0;

/** Cuantos indices se salieron de `viewangletox[4096]`. **La causa.** */
int bmo_addline_fuera = 0;

seg_t*		curline;
side_t*		sidedef;
line_t*		linedef;
sector_t*	frontsector;
sector_t*	backsector;

drawseg_t	drawsegs[MAXDRAWSEGS];
drawseg_t*	ds_p;


void
R_StoreWallRange
( int	start,
  int	stop );




//
// R_ClearDrawSegs
//
void R_ClearDrawSegs (void)
{
    ds_p = drawsegs;
}



//
// ClipWallSegment
// Clips the given range of columns
// and includes it in the new clip list.
//
typedef	struct
{
    int	first;
    int last;
    
} cliprange_t;


#define MAXSEGS		32

// newend is one past the last valid seg
cliprange_t*	newend;
cliprange_t	solidsegs[MAXSEGS];




//
// R_ClipSolidWallSegment
// Does handle solid walls,
//  e.g. single sided LineDefs (middle texture)
//  that entirely block the view.
// 
void
R_ClipSolidWallSegment
( int			first,
  int			last )
{
    cliprange_t*	next;
    cliprange_t*	start;

    // Find the first range that touches the range
    //  (adjacent pixels are touching).
    start = solidsegs;
    while (start->last < first-1)
	start++;

    if (first < start->first)
    {
	if (last < start->first-1)
	{
	    // Post is entirely visible (above start),
	    //  so insert a new clippost.
	    bmo_quien_llamo = 11;
	    R_StoreWallRange (first, last);
	    next = newend;
	    newend++;
	    
	    while (next != start)
	    {
		*next = *(next-1);
		next--;
	    }
	    next->first = first;
	    next->last = last;
	    return;
	}
		
	// There is a fragment above *start.
	bmo_quien_llamo = 12;
	R_StoreWallRange (first, start->first - 1);
	// Now adjust the clip size.
	start->first = first;	
    }

    // Bottom contained in start?
    if (last <= start->last)
	return;			
		
    next = start;
    while (last >= (next+1)->first-1)
    {
	// There is a fragment between two posts.
	bmo_quien_llamo = 13;
	R_StoreWallRange (next->last + 1, (next+1)->first - 1);
	next++;
	
	if (last <= next->last)
	{
	    // Bottom is contained in next.
	    // Adjust the clip size.
	    start->last = next->last;	
	    goto crunch;
	}
    }
	
    // There is a fragment after *next.
    bmo_quien_llamo = 14;
    R_StoreWallRange (next->last + 1, last);
    // Adjust the clip size.
    start->last = last;
	
    // Remove start+1 to next from the clip list,
    // because start now covers their area.
  crunch:
    if (next == start)
    {
	// Post just extended past the bottom of one post.
	return;
    }
    

    while (next++ != newend)
    {
	// Remove a post.
	*++start = *next;
    }

    newend = start+1;
}



//
// R_ClipPassWallSegment
// Clips the given range of columns,
//  but does not includes it in the clip list.
// Does handle windows,
//  e.g. LineDefs with upper and lower texture.
//
void
R_ClipPassWallSegment
( int	first,
  int	last )
{
    cliprange_t*	start;

    // Find the first range that touches the range
    //  (adjacent pixels are touching).
    start = solidsegs;
    while (start->last < first-1)
	start++;

    if (first < start->first)
    {
	if (last < start->first-1)
	{
	    // Post is entirely visible (above start).
	    bmo_quien_llamo = 21;
	    R_StoreWallRange (first, last);
	    return;
	}
		
	// There is a fragment above *start.
	bmo_quien_llamo = 22;
	R_StoreWallRange (first, start->first - 1);
    }

    // Bottom contained in start?
    if (last <= start->last)
	return;			
		
    while (last >= (start+1)->first-1)
    {
	// There is a fragment between two posts.
	bmo_quien_llamo = 23;
	R_StoreWallRange (start->last + 1, (start+1)->first - 1);
	start++;
	
	if (last <= start->last)
	    return;
    }
	
    // There is a fragment after *next.
    bmo_quien_llamo = 24;
    R_StoreWallRange (start->last + 1, last);
}



//
// R_ClearClipSegs
//
void R_ClearClipSegs (void)
{
    solidsegs[0].first = -0x7fffffff;
    solidsegs[0].last = -1;
    solidsegs[1].first = viewwidth;
    solidsegs[1].last = 0x7fffffff;
    newend = solidsegs+2;
}

//
// R_AddLine
// Clips the given segment
// and adds any visible pieces to the line list.
//
void R_AddLine (seg_t*	line)
{
    int			x1;
    int			x2;
    angle_t		angle1;
    angle_t		angle2;
    angle_t		span;
    angle_t		tspan;
    
    curline = line;

    // OPTIMIZE: quickly reject orthogonal back sides.
    angle1 = R_PointToAngle (line->v1->x, line->v1->y);
    angle2 = R_PointToAngle (line->v2->x, line->v2->y);
    
    // Clip to view edges.
    // OPTIMIZE: make constant out of 2*clipangle (FIELDOFVIEW).
    span = angle1 - angle2;
    
    // Back side? I.e. backface culling?
    if (span >= ANG180)
	return;		

    // Global angle needed by segcalc.
    rw_angle1 = angle1;
    angle1 -= viewangle;
    angle2 -= viewangle;
	
    tspan = angle1 + clipangle;
    if (tspan > 2*clipangle)
    {
	tspan -= 2*clipangle;

	// Totally off the left edge?
	if (tspan >= span)
	    return;
	
	angle1 = clipangle;
    }
    tspan = clipangle - angle2;
    if (tspan > 2*clipangle)
    {
	tspan -= 2*clipangle;

	// Totally off the left edge?
	if (tspan >= span)
	    return;	
	angle2 = -clipangle;
    }
    
    // The seg is in the view range,
    // but not necessarily visible.
    angle1 = (angle1+ANG90)>>ANGLETOFINESHIFT;
    angle2 = (angle2+ANG90)>>ANGLETOFINESHIFT;
    /* ** SE MIRA ANTES DE INDEXAR, no despues.
     *
     * Contarlo despues de `viewangletox[angle1]` seria contar habiendo hecho
     * ya la lectura fuera de la tabla -- el instrumento llegaria tarde a lo
     * unico que vino a ver. */
    if (angle1 >= FINEANGLES/2 || angle2 >= FINEANGLES/2)
	bmo_addline_fuera = bmo_addline_fuera + 1;

    x1 = viewangletox[angle1];
    x2 = viewangletox[angle2];

    if (x1 > x2)
	bmo_addline_reves = bmo_addline_reves + 1;

    if (x1 > x2 && bmo_addline_dicho == 0)
    {
	bmo_addline_dicho = 1;
	printf("[bmo] R_AddLine AL REVES: x1=%d x2=%d | fino1=%d fino2=%d | viewangle=%u clipangle=%u
",
	       x1, x2, (int)angle1, (int)angle2,
	       (unsigned int)viewangle, (unsigned int)clipangle);
    }

    // Does not cross a pixel?
    if (x1 == x2)
	return;				
	
    backsector = line->backsector;

    // Single sided line?
    if (!backsector)
	goto clipsolid;		

    // Closed door.
    if (backsector->ceilingheight <= frontsector->floorheight
	|| backsector->floorheight >= frontsector->ceilingheight)
	goto clipsolid;		

    // Window.
    if (backsector->ceilingheight != frontsector->ceilingheight
	|| backsector->floorheight != frontsector->floorheight)
	goto clippass;	
		
    // Reject empty lines used for triggers
    //  and special events.
    // Identical floor and ceiling on both sides,
    // identical light levels on both sides,
    // and no middle texture.
    if (backsector->ceilingpic == frontsector->ceilingpic
	&& backsector->floorpic == frontsector->floorpic
	&& backsector->lightlevel == frontsector->lightlevel
	&& curline->sidedef->midtexture == 0)
    {
	return;
    }
    
				
  clippass:
    R_ClipPassWallSegment (x1, x2-1);	
    return;
		
  clipsolid:
    R_ClipSolidWallSegment (x1, x2-1);
}


//
// R_CheckBBox
// Checks BSP node/subtree bounding box.
// Returns true
//  if some part of the bbox might be visible.
//
int	checkcoord[12][4] =
{
    {3,0,2,1},
    {3,0,2,0},
    {3,1,2,0},
    {0},
    {2,0,2,1},
    {0,0,0,0},
    {3,1,3,0},
    {0},
    {2,0,3,1},
    {2,1,3,1},
    {2,1,3,0}
};


boolean R_CheckBBox (fixed_t*	bspcoord)
{
    int			boxx;
    int			boxy;
    int			boxpos;

    fixed_t		x1;
    fixed_t		y1;
    fixed_t		x2;
    fixed_t		y2;
    
    angle_t		angle1;
    angle_t		angle2;
    angle_t		span;
    angle_t		tspan;
    
    cliprange_t*	start;

    int			sx1;
    int			sx2;
    
    // Find the corners of the box
    // that define the edges from current viewpoint.
    if (viewx <= bspcoord[BOXLEFT])
	boxx = 0;
    else if (viewx < bspcoord[BOXRIGHT])
	boxx = 1;
    else
	boxx = 2;
		
    if (viewy >= bspcoord[BOXTOP])
	boxy = 0;
    else if (viewy > bspcoord[BOXBOTTOM])
	boxy = 1;
    else
	boxy = 2;
		
    boxpos = (boxy<<2)+boxx;
    if (boxpos == 5)
	return true;
	
    x1 = bspcoord[checkcoord[boxpos][0]];
    y1 = bspcoord[checkcoord[boxpos][1]];
    x2 = bspcoord[checkcoord[boxpos][2]];
    y2 = bspcoord[checkcoord[boxpos][3]];
    
    // check clip list for an open space
    angle1 = R_PointToAngle (x1, y1) - viewangle;
    angle2 = R_PointToAngle (x2, y2) - viewangle;
	
    span = angle1 - angle2;

    // Sitting on a line?
    if (span >= ANG180)
	return true;
    
    tspan = angle1 + clipangle;

    if (tspan > 2*clipangle)
    {
	tspan -= 2*clipangle;

	// Totally off the left edge?
	if (tspan >= span)
	    return false;	

	angle1 = clipangle;
    }
    tspan = clipangle - angle2;
    if (tspan > 2*clipangle)
    {
	tspan -= 2*clipangle;

	// Totally off the left edge?
	if (tspan >= span)
	    return false;
	
	angle2 = -clipangle;
    }


    // Find the first clippost
    //  that touches the source post
    //  (adjacent pixels are touching).
    angle1 = (angle1+ANG90)>>ANGLETOFINESHIFT;
    angle2 = (angle2+ANG90)>>ANGLETOFINESHIFT;
    sx1 = viewangletox[angle1];
    sx2 = viewangletox[angle2];

    // Does not cross a pixel.
    if (sx1 == sx2)
	return false;			
    sx2--;
	
    start = solidsegs;
    while (start->last < sx2)
	start++;
    
    if (sx1 >= start->first
	&& sx2 <= start->last)
    {
	// The clippost contains the new span.
	return false;
    }

    return true;
}



//
// R_Subsector
// Determine floor/ceiling planes.
// Add sprites of things in sector.
// Draw one or more line segments.
//
void R_Subsector (int num)
{
    int			count;
    seg_t*		line;
    subsector_t*	sub;
	
#ifdef RANGECHECK
    if (num>=numsubsectors)
	I_Error ("R_Subsector: ss %i with numss = %i",
		 num,
		 numsubsectors);
#endif

    sscount++;
    sub = &subsectors[num];
    frontsector = sub->sector;
    count = sub->numlines;
    line = &segs[sub->firstline];

    if (frontsector->floorheight < viewz)
    {
	floorplane = R_FindPlane (frontsector->floorheight,
				  frontsector->floorpic,
				  frontsector->lightlevel);
    }
    else
	floorplane = NULL;
    
    if (frontsector->ceilingheight > viewz 
	|| frontsector->ceilingpic == skyflatnum)
    {
	ceilingplane = R_FindPlane (frontsector->ceilingheight,
				    frontsector->ceilingpic,
				    frontsector->lightlevel);
    }
    else
	ceilingplane = NULL;
		
    R_AddSprites (frontsector);	

    while (count--)
    {
	R_AddLine (line);
	line++;
    }
}




//
// RenderBSPNode
// Renders all subsectors below a given node,
//  traversing subtree recursively.
// Just call with BSP root.
void R_RenderBSPNode (int bspnum)
{
    node_t*	bsp;
    int		side;

    // Found a subsector?
    if (bspnum & NF_SUBSECTOR)
    {
	if (bspnum == -1)			
	    R_Subsector (0);
	else
	    R_Subsector (bspnum&(~NF_SUBSECTOR));
	return;
    }
		
    bsp = &nodes[bspnum];
    
    // Decide which side the view point is on.
    side = R_PointOnSide (viewx, viewy, bsp);

    // Recursively divide front space.
    R_RenderBSPNode (bsp->children[side]); 

    // Possibly divide back space.
    if (R_CheckBBox (bsp->bbox[side^1]))	
	R_RenderBSPNode (bsp->children[side^1]);
}


